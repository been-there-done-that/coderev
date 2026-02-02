"""Backend tools for Activity Management Agent."""

import datetime
import logging
from typing import Any, Dict, List

from google.adk.tools import ToolContext
from google.adk.tools.function_tool import FunctionTool

from services.database_service import db_service

from ..schemas import (
    ActivityCurrentState,
    ActivityPriority,
    ActivityStatus,
    DrawingStatus,
    TaskType,
)
from ..shared import (
    UIComponentType,
    emit_ui_component,
    extract_context,
    serialize_result,
    validate_pagination,
    validate_string_param,
)

logger = logging.getLogger(__name__)


# ============================================================================
# SQL Constants
# ============================================================================

# Shared User Access Level CTE for RBAC
# Use: user_id as $1, project_id as $2
USER_ACCESS_LEVEL_CTE = """
UserAccessLevel AS (
    SELECT EXISTS (
        SELECT 1
        FROM production.user_organization_role uor
        JOIN production.role_permission rp ON rp.role_id = uor.role_id
        JOIN production.permission p ON p.id = rp.permission_id
        WHERE uor.user_id = $1
        AND p.code_name IN (
            'ACTIVITIES_PROGRESS_UPDATE_ALL_CARDS',
            'FULL_ACCESS',
            'PROJECT_ADMIN'
        )
    ) AS has_broad_access
)
"""

# Access control WHERE clause using UserAccessLevel CTE
# Requires t.roles column containing role assignments
ACCESS_CONTROL_WHERE = """
AND (
    (SELECT has_broad_access FROM UserAccessLevel)
    OR
    EXISTS (
        SELECT 1
        FROM production.user_organization_role uor
        JOIN production.role_permission rp ON rp.role_id = uor.role_id
        JOIN production.permission p ON p.id = rp.permission_id
        WHERE uor.user_id = $1
        AND p.code_name = 'ACTIVITIES_PROGRESS_UPDATE'
        AND t.roles::jsonb @> jsonb_build_array(jsonb_build_object('id', uor.role_id::text))
    )
)
"""


def _build_activity_filters(
    component_name: str | None = None,
    current_state: str | None = None,
    status: str | None = None,
    wbs: str | None = None,
    activity_name: str | None = None,
    task_name: str | None = None,
    priority: str | None = None,
    section: str | None = None,
    task_type: str | None = None,
    module_template_name: str | None = None,
    is_delayed: bool | None = None,
    min_delay_days: int | None = None,
    drawing_status: str | None = None,
    quantity_manager_name: str | None = None,
    assigned_to_me: bool = False,
    raised_by_me: bool = False,
    user_id: str | None = None,
    param_start: int = 3,
) -> tuple[list[str], list]:
    """Build parameterized filter conditions for activity queries.

    Args:
        param_start: Starting parameter number (after user_id=$1, project_id=$2)

    Returns:
        Tuple of (conditions_list, params_list) for parameterized queries
    """
    conditions = []
    params = []
    param_counter = param_start

    # String ILIKE filters (substring match for names/sections)
    ilike_filters = [
        ("component.name", component_name),
        ("component_activity.name", activity_name),
        ("component_activity.task_name", task_name),
        ("component_activity.section", section),
        ("module_template.name", module_template_name),
        ("qm.qm_name", quantity_manager_name),
    ]

    for column, value in ilike_filters:
        if value:
            conditions.append(f"AND {column} ILIKE ${param_counter}")
            params.append(f"%{value}%")
            param_counter += 1

    # WBS filter: exact match + prefix match (not substring)
    # "2.1.5.1.1" matches exactly OR children like "2.1.5.1.1.1"
    # Avoids false matches like "2.1.5.1.10" or "12.1.5.1.1"
    if wbs:
        conditions.append(f"AND (component_activity.wbs = ${param_counter} OR component_activity.wbs LIKE ${param_counter + 1})")
        params.append(wbs)
        params.append(f"{wbs}.%")
        param_counter += 2

    # Exact match filters
    exact_filters = [
        ("component_activity.current_state", current_state),
        ("component_activity.status", status),
        ("component_activity.priority", priority),
        ("component_activity.task_type", task_type),
        ("drawings.drawing_status", drawing_status),
    ]

    for column, value in exact_filters:
        if value:
            conditions.append(f"AND {column} = ${param_counter}")
            params.append(value)
            param_counter += 1

    # Delay filters
    if is_delayed is True:
        conditions.append("AND component_activity.delay > 0")
    elif is_delayed is False:
        conditions.append("AND (component_activity.delay <= 0 OR component_activity.delay IS NULL)")

    if min_delay_days is not None:
        conditions.append(f"AND component_activity.delay >= ${param_counter}")
        params.append(min_delay_days)
        param_counter += 1

    # User-specific filters
    if assigned_to_me and user_id:
        conditions.append(f"AND ${param_counter} = ANY(component_activity.assigned_to)")
        params.append(user_id)
        param_counter += 1

    if raised_by_me and user_id:
        conditions.append(f"AND r.raised_by = ${param_counter}")
        params.append(user_id)
        param_counter += 1

    return conditions, params


@FunctionTool
async def query_activities(
    component_name: str | None = None,
    current_state: ActivityCurrentState | None = None,
    status: ActivityStatus | None = None,
    wbs: str | None = None,
    activity_name: str | None = None,
    task_name: str | None = None,
    priority: ActivityPriority | None = None,
    quantity_manager_name: str | None = None,
    is_delayed: bool | None = None,
    min_delay_days: int | None = None,
    section: str | None = None,
    assigned_to_me: bool | None = None,
    raised_by_me: bool | None = None,
    drawing_status: DrawingStatus | None = None,
    task_type: TaskType | None = None,
    module_template_name: str | None = None,
    group_by_module: bool = False,
    limit: int = 10,
    offset: int = 0,
    *,
    tool_context: ToolContext = None,
) -> Dict[str, Any]:
    """Query activities with comprehensive filtering and RBAC.

    Args:
        component_name: Module name filter (partial match)
        current_state: Activity work state filter
        status: Activity status filter
        wbs: WBS code filter (partial match)
        activity_name: Activity name filter (partial match)
        task_name: Parent task name filter (partial match)
        priority: Activity priority filter
        quantity_manager_name: QM name filter (partial match)
        is_delayed: True=delayed only, False=on-schedule only
        min_delay_days: Minimum delay threshold
        section: Section/area filter (partial match)
        assigned_to_me: Filter activities assigned to current user
        raised_by_me: Filter RFWIs raised by current user
        drawing_status: Drawing upload status filter
        task_type: Task type filter
        module_template_name: Template name filter (partial match)
        group_by_module: Return module counts instead of activities
        limit: Max records (default 10, max 50)
        offset: Pagination offset

    Returns:
        {"status": "success/error", "activities": [...], "record_count": N}
        or {"status": "success", "modules": [...], "total_modules": N} if group_by_module
    """
    # Extract and validate context
    try:
        user_id, project_id = extract_context(tool_context)
    except ValueError as e:
        return {"status": "error", "message": str(e), "activities": []}

    logger.info(f"query_activities called - user_id: {user_id}, project_id: {project_id}")

    # Validate all string parameters using shared utility
    try:
        component_name = validate_string_param("component_name", component_name)
        current_state = validate_string_param("current_state", current_state)
        status = validate_string_param("status", status)
        wbs = validate_string_param("wbs", wbs)
        activity_name = validate_string_param("activity_name", activity_name)
        task_name = validate_string_param("task_name", task_name)
        priority = validate_string_param("priority", priority)
        quantity_manager_name = validate_string_param(
            "quantity_manager_name", quantity_manager_name
        )
        section = validate_string_param("section", section)
        drawing_status = validate_string_param("drawing_status", drawing_status)
        task_type = validate_string_param("task_type", task_type)
        module_template_name = validate_string_param("module_template_name", module_template_name)

        # Validate numeric parameters
        if min_delay_days is not None and not isinstance(min_delay_days, (int, float)):
            raise ValueError("min_delay_days must be a number")

        # Validate pagination
        if not isinstance(limit, int) or limit < 1:
            limit = 20
        if limit > 50:
            limit = 50  # Enforce hard limit
        if not isinstance(offset, int) or offset < 0:
            offset = 0

        # SYT-9811: Auto-increase limit for specific filtered queries to avoid truncation
        # When both component_name and activity_name are specified, user likely wants ALL matches
        if (component_name and activity_name) or (component_name and task_name):
            if limit == 10:  # Only override default, respect explicit higher limits
                limit = 50

    except ValueError as e:
        return {
            "status": "error",
            "message": f"Invalid input parameter: {str(e)}",
            "activities": [],
        }

    # Build filter conditions
    # TODO: SECURITY - Refactor to use parameterized queries via _build_activity_filters()
    # The current f-string interpolation is vulnerable to SQL injection.
    # validate_string_param provides defense-in-depth but is NOT sufficient.
    # See _build_activity_filters() for the intended parameterized approach.
    component_name_filter = (
        f"AND component.name ILIKE '%{component_name}%'" if component_name else ""
    )
    current_state_filter = (
        f"AND component_activity.current_state='{current_state}'"
        if current_state
        else ""
    )
    status_filter = f"AND component_activity.status='{status}'" if status else ""
    # WBS: exact match + prefix match (not substring) to avoid matching "2.1.5.1.10" when searching "2.1.5.1.1"
    wbs_filter = f"AND (component_activity.wbs = '{wbs}' OR component_activity.wbs LIKE '{wbs}.%')" if wbs else ""
    activity_name_filter = (
        f"AND component_activity.name ILIKE '%{activity_name}%'"
        if activity_name
        else ""
    )
    # SYT-9807: Filter by parent task name (e.g., "Shuttering", "Formwork")
    task_name_filter = (
        f"AND component_activity.task_name ILIKE '%{task_name}%'"
        if task_name
        else ""
    )
    priority_filter = (
        f"AND component_activity.priority='{priority}'" if priority else ""
    )
    section_filter = (
        f"AND component_activity.section ILIKE '%{section}%'" if section else ""
    )
    task_type_filter = (
        f"AND component_activity.task_type='{task_type}'" if task_type else ""
    )

    # Module template name filter - joins with template_tasks table
    module_template_name_filter = (
        f"AND module_template.name ILIKE '%{module_template_name}%'" if module_template_name else ""
    )

    # Delay filters
    delay_filter = ""
    if is_delayed is True:
        delay_filter = "AND component_activity.delay > 0"
    elif is_delayed is False:
        delay_filter = (
            "AND (component_activity.delay <= 0 OR component_activity.delay IS NULL)"
        )

    if min_delay_days is not None:
        delay_filter += f" AND component_activity.delay >= {min_delay_days}"

    # Drawing status filter
    drawing_status_filter = ""
    if drawing_status:
        drawing_status_filter = f"AND drawings.drawing_status = '{drawing_status}'"

    # User-specific filters (will be added to WHERE conditions later)
    assigned_to_me_filter = ""
    if assigned_to_me:
        assigned_to_me_filter = f"AND '{user_id}' = ANY(component_activity.assigned_to)"

    raised_by_me_filter = ""
    if raised_by_me:
        raised_by_me_filter = f"AND r.raised_by = '{user_id}'"

    # Quantity manager filter - need to join with users table
    quantity_manager_filter = ""
    if quantity_manager_name:
        quantity_manager_filter = f"AND qm.qm_name ILIKE '%{quantity_manager_name}%'"

    # Check if user is admin or has system-level permissions (skip role filters)
    skip_role_filters = False
    skip_conditions = {"joins": "", "whereConditions": "", "ctes": ""}

    if not db_service.is_initialized:
        return {
            "status": "error",
            "message": "Database service is not available. Please try again later.",
            "activities": [],
        }

    # Check admin status
    check_admin_query = """
    SELECT 
        p.admin_id,
        CASE WHEN p.admin_id = $1 THEN TRUE ELSE FALSE END as is_admin
    FROM production.project p
    WHERE p.id = $2
    """

    admin_check = await db_service.fetch_one(check_admin_query, user_id, project_id)
    if admin_check and admin_check.get("is_admin"):
        skip_role_filters = True

    # Check for lead roles (parameterized query)
    lead_role_sql = """
    SELECT
        p.id,
        uor.user_id,
        uor.role_id,
        per.code_name
    FROM
        production.user_organization_role uor
    LEFT JOIN
        production.project p ON p.organization_id = uor.organization_id
    LEFT JOIN
        production.role_permission rp ON rp.role_id = uor.role_id
    LEFT JOIN
        production.permission per ON per.id = rp.permission_id
    WHERE
        p.id = $1
        AND uor.user_id = $2
    """
    user_roles = await db_service.fetch_all(lead_role_sql, project_id, user_id)
    lead_role = None
    for role in user_roles:
        if role.get("code_name") in ("RFI_LEAD_REQUESTER", "RFI_LEAD_APPROVER"):
            lead_role = role
            break

    # Check for admin-level permissions that should bypass RBAC
    # (matches backend behavior in filter.controller.ts and program-plan.controller.ts)
    if not skip_role_filters:
        admin_permissions = ("FULL_ACCESS", "PROJECT_ADMIN", "ACTIVITIES_PROGRESS_UPDATE_ALL_CARDS")
        for role in user_roles:
            if role.get("code_name") in admin_permissions:
                skip_role_filters = True
                logger.info(f"User {user_id} has {role.get('code_name')} permission - bypassing RBAC filters")
                break

    # Build role-based filter conditions if not admin
    if not skip_role_filters:
        lead_role_id = lead_role.get("role_id") if lead_role else None

        skip_conditions["joins"] = """
        LEFT JOIN UserRoleMatches urm ON component_activity.id = urm.component_activity_id
        LEFT JOIN WorkflowStateStatus ws ON ws.workflow_id = r.workflow_id
        LEFT JOIN distinct_assigned_to distinct_assigned_to on distinct_assigned_to.workflow_id = r.workflow_id
        """

        skip_conditions["ctes"] = f"""
        ,distinct_assigned_to AS (
            SELECT workflow_id, ARRAY_AGG(DISTINCT wsr_assigned_to) AS distinct_assigned_to_array
            FROM rfwi_workflows
            GROUP BY workflow_id
        )

        ,UserRoleMatches AS (
          SELECT
              component_activity.id AS component_activity_id,
              CASE
                  WHEN COUNT(user_org_role.id) > 0 THEN TRUE
                  ELSE FALSE
              END AS user_role_match
          FROM
              production.tasks component_activity
              JOIN LATERAL (
                  SELECT * FROM json_array_elements(component_activity.roles) AS e(role)
                  WHERE json_typeof(component_activity.roles) = 'array'
              ) AS e ON TRUE
              LEFT JOIN production.user_organization_role user_org_role
                  ON (e.role ->> 'id')::varchar = user_org_role.role_id::varchar
                  AND user_org_role.user_id = '{user_id}'
                  AND user_org_role.meta ->> 'organization_long_name' = (e.role ->> 'organization_long_name')::text
                  AND user_org_role.meta ->> 'organization_short_name' = (e.role ->> 'organization_short_name')::text
          where component_activity.task_type in ('ACTIVITY','CHECKPOINT','SUB-CHECKPOINT')
          and component_activity.project_id='{project_id}'
          GROUP BY component_activity.id
      ),
       AggregatedStates AS (
          SELECT
              workflow_id,
              MAX(CASE WHEN execution_state = 'UNLOCKED' THEN 1 ELSE 0 END) AS has_unlocked,
              MAX(CASE WHEN execution_state = 'LOCKED' THEN 1 ELSE 0 END) AS has_locked
          FROM rfwi_workflows
          WHERE
              wsr_assigned_to = '{user_id}' {f"or wsr_assigned_to='{lead_role_id}'" if lead_role_id else ""}
          GROUP BY workflow_id
      ),
        WorkflowStateStatus AS (
          SELECT
          workflow_id,
          CASE
              WHEN has_unlocked = 1 THEN 'ACTION PENDING'
              WHEN has_locked = 1 AND has_unlocked = 0 THEN 'APPROVED'
              ELSE 'ACTION PENDING'
          END AS workflow_state_status
          FROM AggregatedStates
        )
        """

        skip_conditions["whereConditions"] = f"""
        AND
        (
          urm.user_role_match = TRUE
          OR
          r.raised_by = '{user_id}'
          OR
          (
            '{user_id}' = ANY(distinct_assigned_to.distinct_assigned_to_array)
          {f"OR '{lead_role_id}' = ANY(distinct_assigned_to.distinct_assigned_to_array)" if lead_role_id else ""}
          )
        )
        """

    # Handle group_by_module mode - return distinct modules with activity counts
    if group_by_module:
        grouped_sql = f"""
        WITH task_data AS (
            SELECT
                t.*,
                component_task.name AS task_name
            FROM production.tasks t
            LEFT JOIN production.tasks component_task ON component_task.id = t.parent_id
            WHERE
                t.project_id = '{project_id}'
                AND t.deleted = FALSE
                AND t.task_type IN ('ACTIVITY', 'CHECKPOINT')
                AND t.wbs IS NOT NULL
                AND t.current_state IN ('ready_to_work', 'next_in_line', 'completed')
                AND (component_task.id IS NULL OR component_task.task_type in ('TASK'))
        )
        SELECT
            component.id AS module_id,
            component.name AS module_name,
            COUNT(DISTINCT component_activity.id) AS activity_count
        FROM task_data component_activity
        LEFT JOIN production.tasks component
            ON component_activity.component_id = component.id
            AND component.deleted = FALSE
            AND component.project_id = '{project_id}'
        LEFT JOIN production.template_tasks module_template
            ON module_template.id = component.template_id
            AND (module_template.deleted = FALSE OR module_template.deleted IS NULL)
        WHERE component_activity.task_type IN ('ACTIVITY', 'CHECKPOINT', 'SUB-CHECKPOINT')
            AND component_activity.project_id = '{project_id}'
            AND (component_activity.deleted IS NULL OR component_activity.deleted IS NOT TRUE)
            AND component_activity.current_state IN ('ready_to_work', 'next_in_line', 'completed')
            AND (component_activity.work_state = 'RELEASED' OR (component_activity.work_state = 'PAUSED' AND component_activity.status = 'ON_GOING'))
            AND component.id IS NOT NULL
            AND component.name IS NOT NULL
            {component_name_filter}
            {current_state_filter}
            {status_filter}
            {wbs_filter}
            {activity_name_filter}
            {task_name_filter}
            {priority_filter}
            {section_filter}
            {task_type_filter}
            {delay_filter}
            {module_template_name_filter}
            AND component_activity.wbs IS NOT NULL
        GROUP BY component.id, component.name
        ORDER BY component.name
        """

        try:
            logger.info("=" * 80)
            logger.info("QUERY: query_activities (group_by_module=True)")
            logger.info(f"User ID: {user_id}, Project ID: {project_id}")
            logger.info(f"Filters: activity_name={activity_name}, status={status}, is_delayed={is_delayed}, priority={priority}, module_template_name={module_template_name}")
            logger.debug(f"SQL Query:\n{grouped_sql}")
            logger.info("=" * 80)

            results = await db_service.fetch_all(grouped_sql)

            logger.info(f"RESULT: group_by_module query returned {len(results)} modules")

            # Build filter description for message
            filter_descriptions = []
            if activity_name:
                filter_descriptions.append(f"name containing '{activity_name}'")
            if status:
                filter_descriptions.append(f"status '{status}'")
            if is_delayed is True:
                filter_descriptions.append("delayed")
            if priority:
                filter_descriptions.append(f"priority '{priority}'")
            if module_template_name:
                filter_descriptions.append(f"template '{module_template_name}'")
            if component_name:
                filter_descriptions.append(f"module '{component_name}'")

            filter_msg = f" with activities {', '.join(filter_descriptions)}" if filter_descriptions else ""

            return {
                "status": "success",
                "modules": serialize_result(results),
                "total_modules": len(results),
                "message": f"Found {len(results)} modules{filter_msg}.",
            }

        except Exception as e:
            logger.error(f"QUERY ERROR: group_by_module query failed: {str(e)}", exc_info=True)
            return {
                "status": "error",
                "message": f"Database query failed: {str(e)}",
                "modules": [],
                "total_modules": 0,
            }

    # Build the main SQL query (based on activitiest1 endpoint)
    sql_query = f"""
    WITH
    task_data AS (
        SELECT
            t.*,
            component_task.name AS task_name
        FROM production.tasks t
        LEFT JOIN production.tasks component_task ON component_task.id = t.parent_id
        WHERE
            t.project_id = '{project_id}'
            AND t.deleted = FALSE
            AND t.task_type IN ('ACTIVITY', 'CHECKPOINT')
            AND t.wbs IS NOT NULL
            AND t.current_state IN ('ready_to_work', 'next_in_line', 'completed')
            AND (component_task.id IS NULL OR component_task.task_type in ('TASK'))
            AND NOT EXISTS (
                SELECT 1
                FROM production.tasks t_check
                WHERE t_check.parent_id = t.id
                AND t_check.project_id = '{project_id}'
                AND t_check.deleted IS NOT TRUE
                AND t_check.wbs IS NOT NULL
                LIMIT 1
            )
    ),
    rfwi AS (
        SELECT rfwi.*
        FROM production.rfwi rfwi
        JOIN task_data td ON rfwi.task_id = td.id
        WHERE td.status = 'ON_GOING'
        and rfwi.project_id = '{project_id}'
        and rfwi.deleted = false
    ),
    rfwi_workflows AS (
        SELECT
            w.id as workflow_id,
            ws.workflow_id as ws_workflow_id,
            ws.execution_state,
            ws.id as ws_id,
            ws.type as ws_type,
            ws.status as ws_status,
            ws.sequence_no as ws_sequence_no,
            wsr.type as wsr_type,
            wsr.assigned_to as wsr_assigned_to,
            wsr.workflow_state_id,
            wsr.required_status,
            wsr.status,
            wsr.id as wsr_id
        FROM rfwi
        JOIN production.workflow w ON rfwi.workflow_id = w.id
        JOIN production.workflow_state ws ON ws.workflow_id = w.id AND w.project_id = '{
        project_id
    }'
        JOIN production.workflow_state_rule wsr ON wsr.workflow_state_id = ws.id AND w.project_id = '{
        project_id
    }'
        WHERE w.project_id = '{project_id}'
        and rfwi.project_id = '{project_id}'
        and rfwi.deleted = false
    ),
    CurrentStateAggregated AS (
        SELECT
            workflow_id,
            execution_state,
            ws_type,
            ARRAY_REMOVE(ARRAY_AGG(DISTINCT wsr_assigned_to), NULL) AS assigned_to,
            ARRAY_AGG(DISTINCT wsr_type) AS rule_type
        FROM rfwi_workflows
        WHERE rfwi_workflows.execution_state = 'UNLOCKED'
        GROUP BY workflow_id, execution_state, ws_type
    ),
    Drawings AS (
        WITH RECURSIVE task_hierarchy AS (
            SELECT
                t.id,
                t.name,
                t.parent_id,
                t.task_type,
                t.status,
                t.priority,
                t.project_id,
                CASE
                    WHEN t.status = 'UPLOADED' THEN true
                    WHEN t.status = 'NOT_UPLOADED' THEN false
                    ELSE true
                END AS dependency_met
            FROM production.tasks t
            WHERE t.task_type = 'DRAWING_FOLDER'
            AND t.deleted = false
            AND t.project_id = '{project_id}'

            UNION ALL
            SELECT
                t.id,
                t.name,
                t.parent_id,
                t.task_type,
                t.status,
                t.priority,
                t.project_id,
                CASE
                    WHEN t.status = 'UPLOADED' THEN true
                    WHEN t.status = 'NOT_UPLOADED' THEN false
                    WHEN t.status = 'REVISION_IN_PROGRESS' AND t.task_type = 'DRAWING_FILE' AND t.priority = 'HIGH' THEN false
                    ELSE th.dependency_met
                END AS dependency_met
            FROM production.tasks t
            JOIN task_hierarchy th ON t.parent_id = th.id
            WHERE t.task_type = 'DRAWING_FILE'
            AND t.deleted = false
            AND t.project_id = '{project_id}'
        ),
        aggregated_status AS (
            SELECT parent_id AS id, bool_or(dependency_met) AS dependency_met
            FROM task_hierarchy
            GROUP BY parent_id
        ),
        aggregatedFolders AS (
            SELECT
                th.id,
                th.name,
                th.status,
                (ad.dependency_met AND th.dependency_met) AS dependency_met
            FROM task_hierarchy th
            LEFT JOIN aggregated_status ad ON th.id = ad.id
            WHERE th.task_type = 'DRAWING_FOLDER'
        )
        SELECT
            d.target AS component_id,
            -- SYT-9809: Only include name and status, exclude IDs
            json_agg(json_build_object(
                'name', df.name,
                'status', df.status
            )) AS drawing_folders,
            CASE
                WHEN BOOL_OR(df.status = 'UPLOADED') THEN 'UPLOADED'
                WHEN BOOL_OR(df.status = 'REVISION_IN_PROGRESS') THEN 'REVISION_IN_PROGRESS'
                ELSE 'NOT_UPLOADED'
            END AS drawing_status,
            bool_or(df.dependency_met) AS dependency_met
        FROM production.links d
        JOIN aggregatedFolders df ON df.id = d.source
        WHERE d.deleted != true
        and d.project_id = '{project_id}'
        GROUP BY d.target
    ),
    defect_quantity AS (
        SELECT
            d.task_id,
            true AS is_quantity_check_rejected
        FROM production.defect d
        JOIN task_data td on td.id = d.task_id
        WHERE d.type = 'quantity'
        AND d.status = 'RESUBMITTED'
        AND d.deleted != true
        AND d.project_id = '{project_id}'
        GROUP BY d.task_id
    ),
    defect_actionable_comment AS (
        SELECT
            d.rfwi_id,
            true AS defect_exists
        FROM production.defect d
        JOIN task_data td on td.id = d.task_id
        WHERE d.type = 'actionable-comment'
        AND (d.status = 'OPEN' OR d.status = 'RAISED')
        AND d.deleted != true
        AND d.project_id = '{project_id}'
        GROUP BY d.rfwi_id
    ),
    links_target_source AS (
        SELECT DISTINCT ON (l.target)
            l.source,
            l.target
        FROM production.links l
        JOIN task_data td on td.id = l.target AND td.task_type = 'CHECKPOINT'
        WHERE l.target IS NOT NULL
        AND l.deleted = FALSE
        AND l.project_id = '{project_id}'
    ),
    oqca AS (
        SELECT
            lc.source AS component_activity_id,
            true AS has_open_default_rfi
        FROM production.links lc
        JOIN task_data qca ON qca.id = lc.target AND qca.template_task_id IS NULL
        WHERE qca.task_type IN ('ACTIVITY', 'CHECKPOINT', 'SUB-CHECKPOINT')
        AND qca.status != 'COMPLETED'
        AND qca.project_id = '{project_id}'
        GROUP BY lc.source
    ),
    boq_data AS (
        SELECT
            ra.task_id,
            json_agg(json_build_object(
                'boq_id', b.id,
                'boq_name', b.name,
                'resource_id', r.id,
                'resource_name', r.name,
                'resource_type', r.type
            )) AS boq_items
        FROM production.resource_allocation ra
        INNER JOIN production.boq b ON b.id = ra.boq_item_id
        INNER JOIN production.tasks t ON t.id = ra.task_id
        LEFT JOIN production.resource r ON r.id = ra.resource_id
        WHERE ra.deleted != true
        AND ra.boq_item_id IS NOT NULL
        AND t.project_id = '{project_id}'
        AND (b.flow_status IS NULL OR b.flow_status != 'DISABLED')
        GROUP BY ra.task_id
    )
    {skip_conditions["ctes"]}
    , TotalCount AS (
        -- Use DISTINCT to avoid counting duplicates from LEFT JOINs (e.g., activity with 2 RFWIs)
        SELECT COUNT(DISTINCT component_activity.id) as total_count
        FROM task_data component_activity
        LEFT JOIN production.tasks component ON component_activity.component_id = component.id AND component.deleted = FALSE AND component.project_id = '{
        project_id
    }'
        LEFT JOIN production.template_tasks module_template ON module_template.id = component.template_id AND (module_template.deleted = FALSE OR module_template.deleted IS NULL)
        LEFT JOIN links_target_source l ON l.target = component_activity.id
        LEFT JOIN production.tasks checkpointparent ON checkpointparent.id = l.source AND checkpointparent.deleted = FALSE AND checkpointparent.project_id = '{
        project_id
    }'
        LEFT JOIN defect_quantity quantity ON quantity.task_id = component_activity.id
        LEFT JOIN LATERAL (SELECT DISTINCT ON (component_id) * FROM Drawings WHERE drawings.component_id = component.id ) drawings on true
        LEFT JOIN LATERAL (
            SELECT baseline_tasks.*
            FROM production.baselines b
            JOIN production.baseline_tasks baseline_tasks ON baseline_tasks.baseline_id = b.id
            WHERE b.active = true
            AND b.project_id = '{project_id}'
            AND baseline_tasks.task_id = component_activity.id
            LIMIT 1
        ) bt ON TRUE
        LEFT JOIN rfwi r ON (r.task_id = component_activity.id OR r.component_activity_id = component_activity.id)
        LEFT JOIN oqca ON component_activity.id = oqca.component_activity_id AND component_activity.has_default_rfi = true
        LEFT JOIN CurrentStateAggregated current_workflow_state ON component_activity.status = 'ON_GOING' AND current_workflow_state.workflow_id = r.workflow_id
        LEFT JOIN defect_actionable_comment dac ON dac.rfwi_id = r.id
        LEFT JOIN boq_data ON boq_data.task_id = component_activity.id
        LEFT JOIN LATERAL (
            SELECT 
                u.id AS qm_id,
                TRIM(CONCAT(u.first_name, ' ', u.last_name)) AS qm_name,
                u.email AS qm_email
            FROM production.users u
            WHERE u.id = component.quantity_manager_id
            LIMIT 1
        ) qm ON TRUE
        {skip_conditions["joins"]}
        WHERE component_activity.task_type IN ('ACTIVITY', 'CHECKPOINT', 'SUB-CHECKPOINT')
            AND component_activity.project_id = '{project_id}'
            AND (component_activity.deleted IS NULL OR component_activity.deleted IS NOT TRUE)
            AND component_activity.current_state IN ('ready_to_work', 'next_in_line', 'completed')
            AND (component_activity.work_state = 'RELEASED' OR (component_activity.work_state = 'PAUSED' AND component_activity.status = 'ON_GOING'))
            {component_name_filter} {skip_conditions["whereConditions"]}
            {current_state_filter}
            {status_filter}
            {wbs_filter}
            {activity_name_filter}
            {task_name_filter}
            {priority_filter}
            {section_filter}
            {task_type_filter}
            {delay_filter}
            {drawing_status_filter}
            {assigned_to_me_filter}
            {raised_by_me_filter}
            {quantity_manager_filter}
            {module_template_name_filter}
            AND component_activity.wbs IS NOT NULL
    )
    SELECT
        component_activity.id AS id,
        component_activity.component_template_activity_id,
        component_activity.status AS status,
        component_activity.start_time AS actual_start_date,
        component_activity.end_time AS end_time,
        component_activity.name AS name,
        component_activity.task_type AS task_type,
        component_activity.modified_by AS modified_by,
        component_activity.modified_on AS modified_on,
        component_activity.is_checkpoint AS is_checkpoint,
        component_activity.current_state AS current_state,
        component_activity.rfwi_status AS rfwi_status,
        component_activity.planned_duration,
        jsonb_array_length(
        CASE
            WHEN jsonb_typeof(component_activity.comments) = 'array' THEN component_activity.comments
            ELSE '[]'::jsonb
        END
        ) AS comments_count,
        component_activity.comments,
        component_activity.wbs,
        component.id AS component_id,
        component_activity.project_id,
        component_activity.roles,
        component_activity.priority,
        component.name AS component_name,
        component_activity.task_name AS task_name,
        module_template.name AS module_template_name,
        component.quantity_manager_id,
        component.work_state,
        checkpointparent.name AS checkpoint_related_to,
        component_activity.section,
        qm.qm_id,
        qm.qm_name,
        qm.qm_email,
        started_by.started_by_id,
        started_by.started_by_name,
        started_by.started_by_email,
        CASE
            WHEN (component_activity.status = 'NOT_STARTED' OR component_activity.status IS NULL)
                AND component_activity.current_state = 'next_in_line'
                THEN 'next'
            WHEN component_activity.status = 'ON_GOING' THEN 'ongoing'
            WHEN component_activity.status = 'COMPLETED' THEN 'completed'
            WHEN (component_activity.status = 'NOT_STARTED' OR component_activity.status IS NULL)
                AND component_activity.current_state = 'ready_to_work'
                AND component_activity.work_state = 'RELEASED' THEN 'ready'
        END AS derived_status,
        CASE WHEN component_activity.delay IS NOT NULL THEN component_activity.delay * -1 ELSE NULL END AS delay,
        bt.start_date AS planned_start_date,
        CASE
            WHEN bt.end_date IS NULL OR bt.id IS NULL THEN
                CASE
                    WHEN component_activity.planned_duration IS NOT NULL
                        AND component_activity.start_time IS NOT NULL THEN component_activity.start_time + INTERVAL '1 day' * component_activity.planned_duration
                    ELSE NULL
                END
            ELSE bt.end_date
        END AS planned_end_date,
        component_activity.projected_end AS projected_end_date,
        component_activity.projected_end AS projected_end,
        r.id AS rfwi_id,
        r.rfwi_number AS rfwi_number,
        current_workflow_state.rule_type AS type,
        {
        "ws.workflow_state_status"
        if not skip_role_filters
        else '''
          CASE
              WHEN current_workflow_state.execution_state = 'LOCKED' THEN 'APPROVED'
              WHEN current_workflow_state.execution_state = 'UNLOCKED' THEN 'ACTION PENDING'
              ELSE NULL
          END
        '''
    } AS workflow_state_status,
        current_workflow_state.ws_type AS rfwistatetype,
        current_workflow_state.assigned_to AS assigned_to_user,
        COALESCE(dac.defect_exists, false) AS isactionable,
        r.raised_by AS raised_by,
        drawings.drawing_folders AS drawings,
        drawings.drawing_status AS drawing_status,
        COALESCE(quantity.is_quantity_check_rejected, false) AS is_quantity_check_rejected,
        CASE
            WHEN drawings.dependency_met = TRUE THEN FALSE
            WHEN drawings.dependency_met = FALSE THEN TRUE
            ELSE FALSE
        END AS drawing_dependencies,
        COALESCE(oqca.has_open_default_rfi, false) AS has_open_default_rfi,
        boq_data.boq_items AS boq_items,
        (SELECT total_count FROM TotalCount) as total_count
    FROM task_data component_activity
    LEFT JOIN production.tasks component ON component_activity.component_id = component.id AND component.deleted = FALSE AND component.project_id = '{
        project_id
    }'
    LEFT JOIN production.template_tasks module_template ON module_template.id = component.template_id AND (module_template.deleted = FALSE OR module_template.deleted IS NULL)
    LEFT JOIN links_target_source l ON l.target = component_activity.id
    LEFT JOIN production.tasks checkpointparent ON checkpointparent.id = l.source AND checkpointparent.deleted = FALSE AND checkpointparent.project_id = '{
        project_id
    }'
    LEFT JOIN defect_quantity quantity ON quantity.task_id = component_activity.id
    LEFT JOIN LATERAL (SELECT DISTINCT ON (component_id) * FROM Drawings WHERE drawings.component_id = component.id ) drawings on true
    LEFT JOIN LATERAL (
        SELECT baseline_tasks.*
        FROM production.baselines b
        JOIN production.baseline_tasks baseline_tasks ON baseline_tasks.baseline_id = b.id
        WHERE b.active = true
        AND b.project_id = '{project_id}'
        AND baseline_tasks.task_id = component_activity.id
        LIMIT 1
    ) bt ON TRUE
    LEFT JOIN LATERAL (
        SELECT * FROM rfwi r 
        WHERE (r.task_id = component_activity.id OR r.component_activity_id = component_activity.id)
        ORDER BY r.created_on DESC 
        LIMIT 1
    ) r ON TRUE
    LEFT JOIN oqca ON component_activity.id = oqca.component_activity_id AND component_activity.has_default_rfi = true
    LEFT JOIN CurrentStateAggregated current_workflow_state ON component_activity.status = 'ON_GOING' AND current_workflow_state.workflow_id = r.workflow_id
    LEFT JOIN defect_actionable_comment dac ON dac.rfwi_id = r.id
    LEFT JOIN boq_data ON boq_data.task_id = component_activity.id
    LEFT JOIN LATERAL (
        SELECT 
            u.id AS qm_id,
            TRIM(CONCAT(u.first_name, ' ', u.last_name)) AS qm_name,
            u.email AS qm_email
        FROM production.users u
        WHERE u.id = component.quantity_manager_id
        LIMIT 1
    ) qm ON TRUE
    -- SYT-9808: Resolve modified_by UUID to user name for "who started this" queries
    LEFT JOIN LATERAL (
        SELECT
            u.id AS started_by_id,
            TRIM(CONCAT(u.first_name, ' ', u.last_name)) AS started_by_name,
            u.email AS started_by_email
        FROM production.users u
        WHERE u.id = component_activity.modified_by
        LIMIT 1
    ) started_by ON TRUE
    {skip_conditions["joins"]}
    WHERE component_activity.task_type IN ('ACTIVITY', 'CHECKPOINT', 'SUB-CHECKPOINT')
        AND component_activity.project_id = '{project_id}'
        AND (component_activity.deleted IS NULL OR component_activity.deleted IS NOT TRUE)
        AND component_activity.current_state IN ('ready_to_work', 'next_in_line', 'completed')
        AND (component_activity.work_state = 'RELEASED' OR (component_activity.work_state = 'PAUSED' AND component_activity.status = 'ON_GOING'))
        {component_name_filter} {skip_conditions["whereConditions"]}
        {current_state_filter}
        {status_filter}
        {wbs_filter}
        {activity_name_filter}
        {task_name_filter}
        {priority_filter}
        {section_filter}
        {task_type_filter}
        {delay_filter}
        {drawing_status_filter}
        {assigned_to_me_filter}
        {raised_by_me_filter}
        {quantity_manager_filter}
        {module_template_name_filter}
        AND component_activity.wbs IS NOT NULL
    ORDER BY component_activity.modified_on DESC
    LIMIT {limit} OFFSET {offset}
    """

    try:
        # Log query details for debugging
        logger.info("=" * 80)
        logger.info("QUERY: query_activities")
        logger.info(f"User ID: {user_id}, Project ID: {project_id}")
        logger.info(
            f"Filters: component_name={component_name}, current_state={current_state}, status={status}, wbs={wbs}, activity_name={activity_name}, priority={priority}, section={section}, task_type={task_type}, is_delayed={is_delayed}, min_delay_days={min_delay_days}, drawing_status={drawing_status}, assigned_to_me={assigned_to_me}, raised_by_me={raised_by_me}, quantity_manager_name={quantity_manager_name}, module_template_name={module_template_name}"
        )
        logger.info(f"Pagination: limit={limit}, offset={offset}")
        logger.debug(f"SQL Query:\n{sql_query}")
        logger.info("=" * 80)

        results = await db_service.fetch_all(sql_query)

        # Log query results
        logger.info(f"RESULT: query_activities returned {len(results)} records")
        if results:
            logger.debug(f"First record sample: {results[0] if results else 'N/A'}")

        # Build descriptive message based on filters applied
        filter_descriptions = []
        if component_name:
            filter_descriptions.append(f"component '{component_name}'")
        if current_state:
            filter_descriptions.append(f"state '{current_state}'")
        if status:
            filter_descriptions.append(f"status '{status}'")
        if wbs:
            filter_descriptions.append(f"WBS code containing '{wbs}'")
        if activity_name:
            filter_descriptions.append(f"name containing '{activity_name}'")
        if priority:
            filter_descriptions.append(f"priority '{priority}'")
        if quantity_manager_name:
            filter_descriptions.append(f"manager '{quantity_manager_name}'")
        if is_delayed is True:
            filter_descriptions.append("delayed activities")
        elif is_delayed is False:
            filter_descriptions.append("on-schedule activities")
        if min_delay_days is not None:
            filter_descriptions.append(f"delayed by at least {min_delay_days} days")
        if section:
            filter_descriptions.append(f"section '{section}'")
        if assigned_to_me:
            filter_descriptions.append("assigned to me")
        if raised_by_me:
            filter_descriptions.append("raised by me")
        if drawing_status:
            filter_descriptions.append(f"drawing status '{drawing_status}'")
        if task_type:
            filter_descriptions.append(f"type '{task_type}'")
        if module_template_name:
            filter_descriptions.append(f"module template '{module_template_name}'")

        filter_msg = (
            f" matching {', '.join(filter_descriptions)}" if filter_descriptions else ""
        )

    except Exception as e:
        logger.error("=" * 80)
        logger.error("QUERY ERROR: query_activities")
        logger.error(f"User ID: {user_id}, Project ID: {project_id}")
        logger.error(f"Error: {str(e)}")
        logger.error(f"SQL Query:\n{sql_query}")
        logger.error("=" * 80, exc_info=True)
        return {
            "status": "error",
            "message": f"Database query failed: {str(e)}. Please check your filters and try again.",
            "activities": [],
            "record_count": 0,
        }

    # Calculate pagination info
    total_count = results[0].get("total_count", 0) if results else 0
    has_more = (offset + len(results)) < total_count

    # Build concise message - only mention pagination if there are more results
    message = f"Found {total_count} activities{filter_msg}."
    if has_more:
        message += f" Showing {len(results)} of {total_count}. Use offset={offset + limit} to see more."

    return {
        "status": "success",
        "activities": serialize_result(results),
        "record_count": len(results),
        "total_count": total_count,
        "has_more": has_more,
        "message": message,
        "metadata": {
            "user_id": user_id,
            "project_id": project_id,
            "is_admin": bool(admin_check and admin_check.get("is_admin"))
            if "admin_check" in locals()
            else False,
            "skip_role_filters": skip_role_filters,
            "lead_role": lead_role.get("code_name") if lead_role else None,
        },
    }


@FunctionTool
async def query_activity_resources(
    activity_id: str | None = None,
    wbs: str | None = None,
    activity_name: str | None = None,
    *,
    tool_context: ToolContext = None,
) -> Dict[str, Any]:
    """Query resources allocated to specific activities with access control.

    Retrieves detailed resource allocation information for activities matching the provided filters.
    Only returns data for activities the user is authorized to access.

    Args:
        activity_id (str, optional): Unique identifier of the activity.
        wbs (str, optional): Work Breakdown Structure code (exact or partial match).
        activity_name (str, optional): Name of the activity (partial match).
        tool_context (ToolContext): Framework-provided context.

    Returns:
        dict: Dictionary containing 'status', 'resources' list, and 'message'.
    """
    if tool_context is None:
        return {
            "status": "error",
            "message": "Tool context is missing.",
            "resources": [],
        }

    user_id = tool_context.state.get("user_id")
    project_id = tool_context.state.get("project_id")

    if not user_id or not project_id:
        return {
            "status": "error",
            "message": "User ID or Project ID missing from context.",
            "resources": [],
        }

    # Input validation
    if not any([activity_id, wbs, activity_name]):
        return {
            "status": "error",
            "message": "Please provide at least one filter: activity_id, wbs, or activity_name.",
            "resources": [],
        }

    # Build query with RBAC - aggregated by activity, resource, and contractor
    sql_query = """
    SELECT
        t.id as activity_id,
        t.name as activity_name,
        t.wbs,
        r.name as resource_name,
        r.type as resource_type,
        r.unit,
        SUM(COALESCE(ra.estimate, 0)) as estimated_quantity,
        c.name as contractor_name,
        b.name as boq_item_name,
        COUNT(ra.id) as allocation_count
    FROM production.resource_allocation ra
    JOIN production.tasks t ON t.id = ra.task_id
    JOIN production.project p ON p.id = t.project_id
    LEFT JOIN production.boq_item_resources bir ON bir.id = ra.boq_item_resource_id
    LEFT JOIN production.resource r ON r.id = bir.resource_id
    LEFT JOIN production.contractors c ON c.id = bir.contractor_id
    LEFT JOIN production.boq b ON b.id = ra.boq_item_id
    LEFT JOIN LATERAL (
            SELECT role FROM jsonb_array_elements(
                CASE
                    WHEN jsonb_typeof(t.roles::jsonb) = 'array' THEN t.roles::jsonb
                    ELSE '[]'::jsonb
                END
            ) AS role
        ) AS e ON TRUE
    LEFT JOIN production.user_organization_role user_org_role
        ON (e.role ->> 'id')::varchar = user_org_role.role_id::varchar
        AND user_org_role.user_id = $1
        AND user_org_role.meta ->> 'organization_long_name' = (e.role ->> 'organization_long_name')::text
        AND user_org_role.meta ->> 'organization_short_name' = (e.role ->> 'organization_short_name')::text
    LEFT JOIN production.role_permission rp on rp.role_id = (e.role ->> 'id')::uuid
    LEFT JOIN production.permission permissions on permissions.id = rp.permission_id
    WHERE t.project_id = $2
    AND t.deleted = false
    AND ra.deleted = false
    AND (
        permissions.code_name IN ('PROJECT_ADMIN','FULL_ACCESS','ACTIVITIES_PROGRESS_UPDATE')
        OR p.admin_id = $1
        OR user_org_role.id IS NOT NULL
    )
    """

    params = [user_id, project_id]
    param_idx = 3

    if activity_id:
        sql_query += f" AND t.id = ${param_idx}"
        params.append(activity_id)
        param_idx += 1

    if wbs:
        # WBS: exact match + prefix match (not substring) to avoid matching "2.1.5.1.10" when searching "2.1.5.1.1"
        sql_query += f" AND (t.wbs = ${param_idx} OR t.wbs LIKE ${param_idx + 1})"
        params.append(wbs)
        params.append(f"{wbs}.%")
        param_idx += 2

    if activity_name:
        sql_query += f" AND t.name ILIKE ${param_idx}"
        params.append(f"%{activity_name}%")
        param_idx += 1

    sql_query += """
    GROUP BY t.id, t.name, t.wbs, r.name, r.type, r.unit, c.name, b.name
    ORDER BY t.wbs, r.name, c.name
    """

    try:
        from services.database_service import db_service

        if not db_service.is_initialized:
            return {
                "status": "error",
                "message": "Database service not initialized.",
                "resources": [],
            }

        # Log query details
        logger.info("=" * 80)
        logger.info("QUERY: query_activity_resources")
        logger.info(f"User ID: {user_id}, Project ID: {project_id}")
        logger.info(
            f"Filters: activity_id={activity_id}, wbs={wbs}, activity_name={activity_name}"
        )
        logger.info(f"Params: {params}")
        logger.debug(f"SQL Query:\n{sql_query}")
        logger.info("=" * 80)

        results = await db_service.fetch_all(sql_query, *params)

        # Log results
        logger.info(f"RESULT: query_activity_resources returned {len(results)} records")
        if results:
            logger.debug(f"First record sample: {results[0] if results else 'N/A'}")

        # Calculate meaningful counts for user message
        distinct_resources = len(set(r.get("resource_name") for r in results if r.get("resource_name")))
        total_allocations = sum(r.get("allocation_count", 1) for r in results)

        return {
            "status": "success",
            "resources": serialize_result(results),
            "distinct_resources": distinct_resources,
            "total_allocations": total_allocations,
            "message": f"Found {distinct_resources} resource(s) with {len(results)} vendor allocation(s).",
        }

    except Exception as e:
        logger.error("=" * 80)
        logger.error("QUERY ERROR: query_activity_resources")
        logger.error(f"User ID: {user_id}, Project ID: {project_id}")
        logger.error(f"Error: {str(e)}")
        logger.error(f"SQL Query:\n{sql_query}")
        logger.error(f"Params: {params}")
        logger.error("=" * 80, exc_info=True)
        return {
            "status": "error",
            "message": f"Database query failed: {str(e)}",
            "resources": [],
        }


# =============================================================================
# HITL (Human-in-the-Loop) Confirmation Tools
# These tools use ADK's native request_confirmation() for user confirmation
# =============================================================================


@FunctionTool
async def log_activity_consumption(
    activity_id: str,
    activity_name: str,
    entries: List[Dict[str, Any]],
    *,
    tool_context: ToolContext = None,
) -> Dict[str, Any]:
    """Log resource consumption/progress for an activity with user confirmation.

    This tool requests user confirmation before logging consumption entries.
    Records resource usage quantities for activities including labor, materials, equipment.

    Each entry in the entries list should contain:
    - boq_item_resource_id: BOQ item resource ID (production.boq_item_resources.id)
    - template_task_id: Template task ID (production.tasks.template_task_id)
    - template_id: Template ID (production.tasks.template_id)
    - component_id: Component ID (production.component.id)
    - consumption: Consumption value (quantity or decimal for percentage mode)
    - consumption_percent: Optional percentage value
    - date: Optional ISO date string for consumption date

    Use this tool when the user wants to:
    - Log progress on an activity
    - Record material consumption
    - Log labor hours or equipment usage

    Args:
        activity_id: Activity/task ID (production.tasks.id)
        activity_name: Activity name for display in confirmation UI
        entries: List of consumption entries to log
        tool_context: ADK framework-provided context

    Returns:
        dict: Status of the operation (awaiting_confirmation, success, rejected, or error)
    """
    if tool_context is None:
        return {"status": "error", "message": "Tool context is missing."}

    user_id = tool_context.state.get("user_id")
    project_id = tool_context.state.get("project_id")

    if not user_id or not project_id:
        return {
            "status": "error",
            "message": "User ID or Project ID missing from context.",
        }

    if not entries or len(entries) == 0:
        return {
            "status": "error",
            "message": "At least one consumption entry is required.",
        }

    # Check if we have confirmation from the user
    if not tool_context.tool_confirmation:
        # Request confirmation from user
        tool_context.request_confirmation(
            hint="Please confirm the consumption entries to log",
            payload={
                "tool_name": "log_activity_consumption",
                "activity_id": activity_id,
                "activity_name": activity_name,
                "entries": entries,
                "entry_count": len(entries),
                "confirmed": False,
            },
        )
        return {"status": "awaiting_confirmation"}

    # User has responded - check if confirmed
    confirmation_payload = tool_context.tool_confirmation.payload
    if not confirmation_payload.get("confirmed"):
        return {
            "status": "rejected",
            "message": "User cancelled the consumption logging.",
        }

    # Use potentially modified entries from confirmation
    final_entries = confirmation_payload.get("entries", entries)

    try:
        from services.database_service import db_service

        if not db_service.is_initialized:
            return {"status": "error", "message": "Database service not initialized."}

        logged_count = 0
        for entry in final_entries:
            insert_sql = """
            INSERT INTO production.consumption_entries (
                id,
                task_id,
                boq_item_resource_id,
                template_task_id,
                template_id,
                component_id,
                consumption,
                consumption_percent,
                date,
                created_by,
                created_on,
                project_id
            ) VALUES (
                gen_random_uuid(),
                $1, $2, $3, $4, $5, $6, $7, $8, $9, NOW(), $10
            )
            RETURNING id
            """

            consumption_date = entry.get("date") or datetime.date.today().isoformat()

            result = await db_service.fetch_one(
                insert_sql,
                activity_id,
                entry.get("boq_item_resource_id"),
                entry.get("template_task_id"),
                entry.get("template_id"),
                entry.get("component_id"),
                entry.get("consumption"),
                entry.get("consumption_percent"),
                consumption_date,
                user_id,
                project_id,
            )
            if result:
                logged_count += 1

        logger.info(
            f"Logged {logged_count} consumption entries for activity {activity_id} by {user_id}"
        )

        return {
            "status": "success",
            "message": f"Logged {logged_count} consumption entries for '{activity_name}'.",
            "logged_count": logged_count,
            "consumption_payload": final_entries,  # Includes full payload for history logging
        }

    except Exception as e:
        logger.error(f"Error logging consumption: {e}", exc_info=True)
        return {"status": "error", "message": f"Failed to log consumption: {str(e)}"}


@FunctionTool
async def assign_activity(
    activity_id: str,
    activity_name: str,
    new_assignee_id: str,
    new_assignee_name: str,
    current_assignee_id: str | None = None,
    current_assignee_name: str | None = None,
    comment: str | None = None,
    *,
    tool_context: ToolContext = None,
) -> Dict[str, Any]:
    """Assign or reassign an activity to a user with confirmation.

    This tool requests user confirmation before changing the activity assignment.
    Changes the assigned user for an activity with optional comment.

    Use this tool when the user wants to:
    - Assign an activity to someone
    - Reassign an activity to a different user
    - Take ownership of an activity

    Args:
        activity_id: Activity/task ID (production.tasks.id)
        activity_name: Activity name for display in confirmation UI
        new_assignee_id: New assignee user ID (production.users.id)
        new_assignee_name: New assignee name for display
        current_assignee_id: Current assignee user ID (if any)
        current_assignee_name: Current assignee name for display
        comment: Optional comment for the assignment change
        tool_context: ADK framework-provided context

    Returns:
        dict: Status of the operation (awaiting_confirmation, success, rejected, or error)
    """
    if tool_context is None:
        return {"status": "error", "message": "Tool context is missing."}

    user_id = tool_context.state.get("user_id")
    project_id = tool_context.state.get("project_id")

    if not user_id or not project_id:
        return {
            "status": "error",
            "message": "User ID or Project ID missing from context.",
        }

    # Check if we have confirmation from the user
    if not tool_context.tool_confirmation:
        # Request confirmation from user
        tool_context.request_confirmation(
            hint="Please confirm the activity assignment",
            payload={
                "tool_name": "assign_activity",
                "activity_id": activity_id,
                "activity_name": activity_name,
                "current_assignee_id": current_assignee_id,
                "current_assignee_name": current_assignee_name,
                "new_assignee_id": new_assignee_id,
                "new_assignee_name": new_assignee_name,
                "comment": comment,
                "confirmed": False,
            },
        )
        return {"status": "awaiting_confirmation"}

    # User has responded - check if confirmed
    confirmation_payload = tool_context.tool_confirmation.payload
    if not confirmation_payload.get("confirmed"):
        return {"status": "rejected", "message": "User cancelled the assignment."}

    # Use potentially modified values from confirmation
    final_assignee_id = confirmation_payload.get("new_assignee_id", new_assignee_id)
    final_comment = confirmation_payload.get("comment", comment)

    try:
        from services.database_service import db_service

        if not db_service.is_initialized:
            return {"status": "error", "message": "Database service not initialized."}

        # Update the activity assignment (add to assigned_to array)
        update_sql = """
        UPDATE production.tasks
        SET assigned_to = CASE
                WHEN assigned_to IS NULL THEN ARRAY[$1::uuid]
                WHEN NOT ($1::uuid = ANY(assigned_to)) THEN array_append(assigned_to, $1::uuid)
                ELSE assigned_to
            END,
            modified_by = $2,
            modified_on = NOW()
        WHERE id = $3
        AND project_id = $4
        AND deleted = false
        RETURNING id, name, assigned_to
        """

        result = await db_service.fetch_one(
            update_sql, final_assignee_id, user_id, activity_id, project_id
        )

        if not result:
            return {
                "status": "error",
                "message": f"Activity '{activity_id}' not found or access denied.",
            }

        # Log the assignment change in comments if comment provided
        if final_comment:
            comment_sql = """
            UPDATE production.tasks
            SET comments = COALESCE(comments, '[]'::jsonb) || $1::jsonb
            WHERE id = $2 AND project_id = $3
            """
            comment_entry = {
                "text": f"Assigned to {confirmation_payload.get('new_assignee_name', new_assignee_name)}: {final_comment}",
                "user_id": user_id,
                "timestamp": datetime.datetime.now(datetime.timezone.utc).isoformat(),
            }
            await db_service.execute(
                comment_sql,
                f"[{__import__('json').dumps(comment_entry)}]",
                activity_id,
                project_id,
            )

        logger.info(
            f"Activity {activity_id} assigned to {final_assignee_id} by {user_id}"
        )

        return {
            "status": "success",
            "message": f"Activity '{activity_name}' assigned to {confirmation_payload.get('new_assignee_name', new_assignee_name)}.",
            "result": serialize_result(result),
        }

    except Exception as e:
        logger.error(f"Error assigning activity: {e}", exc_info=True)
        return {"status": "error", "message": f"Failed to assign activity: {str(e)}"}


# ============================================================================
# Direct UI Emission Tools
# ============================================================================

# SQL for activity status detail query with proper RBAC
ACTIVITY_STATUS_SQL = """
WITH UserAccessLevel AS (
    SELECT EXISTS (
        SELECT 1
        FROM production.user_organization_role uor
        JOIN production.role_permission rp ON rp.role_id = uor.role_id
        JOIN production.permission p ON p.id = rp.permission_id
        WHERE uor.user_id = $1
        AND p.code_name IN (
            'ACTIVITIES_PROGRESS_UPDATE_ALL_CARDS',
            'FULL_ACCESS',
            'PROJECT_ADMIN'
        )
    ) AS has_broad_access
),
ActiveBaseline AS (
    SELECT b.id AS baseline_id
    FROM production.baselines b
    WHERE b.project_id = $2 AND b.active = true
    LIMIT 1
),
-- Check if module has drawings attached via links table
drawings AS (
    SELECT
        l.target AS component_id,
        COUNT(l.id) > 0 AS has_drawings
    FROM production.links l
    JOIN production.tasks df ON df.id = l.source AND df.task_type = 'DRAWING_FOLDER' AND df.deleted = false
    WHERE l.deleted = false
    GROUP BY l.target
)
SELECT
    t.id,
    t.name,
    t.wbs,
    t.status,
    t.current_state,
    t.priority,
    t.planned_duration,
    -- Planned dates from baseline
    bt.start_date AS planned_start_date,
    CASE
        WHEN bt.end_date IS NOT NULL THEN bt.end_date
        WHEN t.planned_duration IS NOT NULL AND t.start_time IS NOT NULL
            THEN t.start_time + INTERVAL '1 day' * t.planned_duration
        ELSE NULL
    END AS planned_end_date,
    t.projected_end AS projected_end_date,
    t.start_time AS actual_start_date,
    t.end_time,
    CASE WHEN t.delay IS NOT NULL THEN t.delay * -1 ELSE NULL END AS delay,
    t.component_id,
    component.name AS component_name,
    -- Module template name for modulePath
    module_template.name AS module_template_name,
    -- User who last modified (working on activity)
    modified_by.id AS started_by_id,
    TRIM(CONCAT(modified_by.first_name, ' ', modified_by.last_name)) AS started_by_name,
    modified_by.email AS started_by_email,
    -- Quantity manager
    qm.id AS qm_id,
    TRIM(CONCAT(qm.first_name, ' ', qm.last_name)) AS qm_name,
    qm.email AS qm_email,
    COALESCE(d.has_drawings, false) AS has_drawings
FROM production.tasks t
LEFT JOIN production.tasks component ON component.id = t.component_id AND component.deleted = false
LEFT JOIN production.template_tasks module_template ON module_template.id = component.template_id
LEFT JOIN production.users modified_by ON modified_by.id = t.modified_by
LEFT JOIN production.users qm ON qm.id = component.quantity_manager_id
LEFT JOIN ActiveBaseline ab ON true
LEFT JOIN production.baseline_tasks bt ON bt.baseline_id = ab.baseline_id AND bt.task_id = t.id
LEFT JOIN drawings d ON d.component_id = t.component_id
WHERE t.project_id = $2
    AND t.deleted = false
    AND t.task_type IN ('ACTIVITY', 'CHECKPOINT', 'SUB-CHECKPOINT')
    AND t.wbs IS NOT NULL
    AND (
        (SELECT has_broad_access FROM UserAccessLevel)
        OR
        EXISTS (
            SELECT 1
            FROM production.user_organization_role uor
            JOIN production.role_permission rp ON rp.role_id = uor.role_id
            JOIN production.permission p ON p.id = rp.permission_id
            WHERE uor.user_id = $1
            AND p.code_name = 'ACTIVITIES_PROGRESS_UPDATE'
            AND t.roles::jsonb @> jsonb_build_array(jsonb_build_object('id', uor.role_id::text))
        )
    )
"""


@FunctionTool
async def get_activity_status(
    activity_id: str | None = None,
    wbs: str | None = None,
    activity_name: str | None = None,
    *,
    tool_context: ToolContext = None,
) -> Dict[str, Any]:
    """Get activity status and emit activity_status_detail UI component.

    Search by activity_id (exact), wbs (partial), or activity_name (partial).
    If multiple matches found, emits activity_carousel for selection.
    When user selects from carousel, automatically emits activity_status_detail.

    Args:
        activity_id: Exact activity UUID
        wbs: WBS code (partial match)
        activity_name: Activity name (partial match)

    Returns:
        {"status": "ui_rendered", "component": "activity_status_detail"}
    """
    try:
        user_id, project_id = extract_context(tool_context)
    except ValueError as e:
        return {"status": "error", "message": str(e)}

    if not db_service.is_initialized:
        return {"status": "error", "message": "Database service not available."}

    # Handle confirmation response (user selected from carousel)
    if tool_context.tool_confirmation:
        confirmation_payload = tool_context.tool_confirmation.payload
        # Extract selected activity_id from carousel selection
        selected_activity_id = confirmation_payload.get("activityId") or confirmation_payload.get("activity_id")
        if selected_activity_id:
            logger.info(f"Carousel selection received - fetching activity detail for: {selected_activity_id}")
            return await _fetch_and_emit_activity_detail(
                selected_activity_id, user_id, project_id, tool_context
            )
        else:
            logger.warning(f"Carousel confirmation missing activityId: {confirmation_payload}")
            # Fall through to normal search if no activity_id in confirmation

    if not any([activity_id, wbs, activity_name]):
        return {
            "status": "error",
            "message": "Please provide activity_id, wbs, or activity_name to search.",
        }

    # Build query with filters
    conditions = []
    params = [user_id, project_id]
    param_counter = 3

    if activity_id:
        conditions.append(f"AND t.id = ${param_counter}")
        params.append(activity_id)
        param_counter += 1
    if wbs:
        # Exact match first, then prefix match for children (not substring match)
        # "2.1.5.1.1" matches exactly OR children like "2.1.5.1.1.1"
        # Avoids false matches like "2.1.5.1.10" or "12.1.5.1.1"
        conditions.append(f"AND (t.wbs = ${param_counter} OR t.wbs LIKE ${param_counter + 1})")
        params.append(wbs)
        params.append(f"{wbs}.%")
        param_counter += 2
    if activity_name:
        conditions.append(f"AND t.name ILIKE ${param_counter}")
        params.append(f"%{activity_name}%")
        param_counter += 1

    sql_query = ACTIVITY_STATUS_SQL + "\n".join(conditions) + "\nLIMIT 10"

    try:
        results = await db_service.fetch_all(sql_query, *params)

        if not results:
            return {
                "status": "not_found",
                "message": "No activities found matching your criteria.",
            }

        if len(results) > 1:
            # Multiple matches - emit carousel for selection
            carousel_payload = {
                "activities": [
                    {
                        "id": str(r["id"]),
                        "wbsNumber": r["wbs"],
                        "description": r["name"],
                        "modulePath": [r["component_name"]] if r.get("component_name") else [],
                    }
                    for r in results
                ],
                "header_text": "I found multiple matching activities. Please select one to continue.",
            }
            emit_ui_component(
                tool_context,
                UIComponentType.ACTIVITY_CAROUSEL,
                carousel_payload,
                requires_response=True,
            )
            return {
                "status": "ui_rendered",
                "component": "activity_carousel",
                "count": len(results),
            }

        # Single match - emit status detail
        activity = results[0]
        status_payload = _build_activity_status_payload(activity)

        emit_ui_component(
            tool_context,
            UIComponentType.ACTIVITY_STATUS_DETAIL,
            status_payload,
            requires_response=False,
        )

        return {
            "status": "ui_rendered",
            "component": "activity_status_detail",
            "activity_name": activity["name"],
        }

    except Exception as e:
        logger.error(f"Error fetching activity status: {e}", exc_info=True)
        return {"status": "error", "message": f"Database error: {str(e)}"}


async def _fetch_and_emit_activity_detail(
    activity_id: str,
    user_id: str,
    project_id: str,
    tool_context: ToolContext,
) -> Dict[str, Any]:
    """Fetch a specific activity by ID and emit activity_status_detail component.

    Args:
        activity_id: The activity UUID to fetch
        user_id: Current user ID for RBAC
        project_id: Current project ID
        tool_context: ADK ToolContext for emitting UI component

    Returns:
        {"status": "ui_rendered", "component": "activity_status_detail", "activity_name": "..."}
    """
    sql_query = ACTIVITY_STATUS_SQL + "\nAND t.id = $3\nLIMIT 1"

    try:
        results = await db_service.fetch_all(sql_query, user_id, project_id, activity_id)

        if not results:
            return {
                "status": "not_found",
                "message": f"Activity with ID {activity_id} not found or you don't have access.",
            }

        activity = results[0]
        status_payload = _build_activity_status_payload(activity)

        emit_ui_component(
            tool_context,
            UIComponentType.ACTIVITY_STATUS_DETAIL,
            status_payload,
            requires_response=False,
        )

        return {
            "status": "ui_rendered",
            "component": "activity_status_detail",
            "activity_name": activity["name"],
        }

    except Exception as e:
        logger.error(f"Error fetching activity detail for {activity_id}: {e}", exc_info=True)
        return {"status": "error", "message": f"Database error: {str(e)}"}


def _build_activity_status_payload(activity: dict) -> dict:
    """Transform database activity row to ActivityStatusDetailPayload format."""
    status = activity.get("status", "NOT_STARTED")
    delay = activity.get("delay")
    activity_name = activity.get("name", "the activity")

    # Build dates based on status
    dates = {
        "plannedStartDate": _format_date(activity.get("planned_start_date")),
        "plannedEndDate": _format_date(activity.get("planned_end_date")),
        "projectedEndDate": _format_date(activity.get("projected_end_date")),
    }

    if status == "ON_GOING":
        dates["startDate"] = _format_date(activity.get("actual_start_date"))
        dates["endDate"] = _format_date(
            activity.get("projected_end_date") or activity.get("planned_end_date")
        )
    elif status == "COMPLETED":
        dates["completedDate"] = _format_date(activity.get("end_time"))

    # Build startedBy info (user who started the activity)
    started_by = None
    if activity.get("started_by_id"):
        started_by = {
            "id": str(activity["started_by_id"]),
            "name": activity.get("started_by_name") or "Unknown",
            "email": activity.get("started_by_email"),
        }

    # Build quantityManager info
    quantity_manager = None
    if activity.get("qm_id"):
        quantity_manager = {
            "id": str(activity["qm_id"]),
            "name": activity.get("qm_name") or "Unknown",
            "email": activity.get("qm_email"),
        }

    # assignedTo: For ongoing/completed, use startedBy; for not_started, use quantityManager
    assigned_to = started_by if started_by else quantity_manager

    # Build modulePath - include module template name if available
    module_path = []
    if activity.get("module_template_name"):
        module_path.append(activity["module_template_name"])
    if activity.get("component_name"):
        module_path.append(activity["component_name"])

    # Determine available actions based on status and conditions
    has_drawings = activity.get("has_drawings", False)
    actions = []
    if status == "NOT_STARTED":
        actions.append({"id": "start_activity", "label": "Start Activity"})
        if has_drawings:
            actions.append({"id": "drawing_details", "label": "Drawing Details"})
    elif status == "ON_GOING":
        actions.append({"id": "boq_log", "label": "BOQ Log"})
        if has_drawings:
            actions.append({"id": "drawing_details", "label": "Drawing Details"})
        actions.append({"id": "view_report", "label": "View Report"})
    else:  # COMPLETED
        actions.append({"id": "boq_log", "label": "BOQ Log"})
        if has_drawings:
            actions.append({"id": "drawing_details", "label": "Drawing Details"})
        actions.append({"id": "view_report", "label": "View Report"})

    # Build delay info
    delay_info = None
    if delay is not None:
        # Delay < 0 means behind schedule (delayed)
        is_delayed = delay < 0
        delay_days = abs(int(delay)) if delay else 0
        delay_info = {
            "isDelayed": is_delayed,
            "delayDays": delay_days if is_delayed else None,
            "message": f"Delayed by {delay_days} days from the planned schedule" if is_delayed else None,
        }

    # Generate contextual message_text based on status
    message_text = _generate_activity_message(activity_name, status, delay_info)

    return {
        "activity": {
            "activityId": str(activity["id"]),
            "activityCode": activity.get("wbs", ""),
            "activityName": activity_name,
            "moduleId": str(activity["component_id"]) if activity.get("component_id") else None,
            "moduleName": activity.get("component_name"),
            "modulePath": module_path,
            "status": status,
            "durationDays": int(activity["planned_duration"]) if activity.get("planned_duration") else None,
            "startedBy": started_by,
            "quantityManager": quantity_manager,
            "assignedTo": assigned_to,
            "dates": dates,
            "delayInfo": delay_info,
            "hasDrawings": activity.get("has_drawings", False),
            "availableActions": actions,
        },
        "header_text": f"Here's the current status for {activity_name}",
        "message_text": message_text,
    }


def _generate_activity_message(activity_name: str, status: str, delay_info: dict | None) -> str:
    """Generate contextual message based on activity status and delay."""
    if status == "COMPLETED":
        return f"{activity_name} has been completed and marked as closed."
    elif status == "ON_GOING":
        if delay_info and delay_info.get("isDelayed"):
            delay_days = delay_info.get("delayDays", 0)
            return f"{activity_name} is currently in progress but is delayed by {delay_days} days."
        return f"{activity_name} is currently in progress with consumption being tracked."
    else:  # NOT_STARTED
        if delay_info and delay_info.get("isDelayed"):
            delay_days = delay_info.get("delayDays", 0)
            return f"{activity_name} has not started and is currently delayed by {delay_days} days from the planned schedule."
        return f"{activity_name} is ready to start."


def _format_date(date_value) -> str | None:
    """Format date value to ISO string."""
    if date_value is None:
        return None
    if isinstance(date_value, str):
        return date_value
    if hasattr(date_value, "isoformat"):
        return date_value.isoformat()
    return str(date_value)


def _get_wbs_fallback_levels(wbs: str) -> List[str]:
    """Generate progressive WBS fallback patterns.

    Example: "1.1.1.1.5.5." -> ["1.1.1.1.5.5", "1.1.1.1.5", "1.1.1.1", "1.1"]

    Args:
        wbs: Original WBS string (may have trailing dot)

    Returns:
        List of WBS patterns from most specific to least specific (min 2 levels)
    """
    # Remove trailing dots and normalize
    clean_wbs = wbs.rstrip('.')

    # Split by dots
    parts = clean_wbs.split('.')

    # Generate fallback levels (keep at least 2 levels)
    fallback_levels = []
    for i in range(len(parts), 1, -1):
        fallback_levels.append('.'.join(parts[:i]))

    return fallback_levels


# SQL for activity carousel query
ACTIVITY_CAROUSEL_SQL = """
WITH UserAccessLevel AS (
    SELECT EXISTS (
        SELECT 1
        FROM production.user_organization_role uor
        JOIN production.role_permission rp ON rp.role_id = uor.role_id
        JOIN production.permission p ON p.id = rp.permission_id
        WHERE uor.user_id = $1
        AND p.code_name IN (
            'ACTIVITIES_PROGRESS_UPDATE_ALL_CARDS',
            'FULL_ACCESS',
            'PROJECT_ADMIN'
        )
    ) AS has_broad_access
)
SELECT
    t.id,
    t.name,
    t.wbs,
    t.status,
    t.current_state,
    t.work_state,
    component.name AS component_name,
    template.name AS template_name
FROM production.tasks t
LEFT JOIN production.tasks component ON component.id = t.component_id
LEFT JOIN production.template_tasks template ON template.id = component.template_id
WHERE t.project_id = $2
    AND t.deleted = false
    AND t.task_type IN ('ACTIVITY', 'CHECKPOINT', 'SUB-CHECKPOINT')
    AND t.wbs IS NOT NULL
    AND t.current_state IN ('ready_to_work', 'next_in_line', 'completed')
    AND (t.work_state = 'RELEASED' OR (t.work_state = 'PAUSED' AND t.status = 'ON_GOING'))
    AND (
        (SELECT has_broad_access FROM UserAccessLevel)
        OR
        EXISTS (
            SELECT 1
            FROM production.user_organization_role uor
            JOIN production.role_permission rp ON rp.role_id = uor.role_id
            JOIN production.permission p ON p.id = rp.permission_id
            WHERE uor.user_id = $1
            AND p.code_name = 'ACTIVITIES_PROGRESS_UPDATE'
            AND t.roles::jsonb @> jsonb_build_array(jsonb_build_object('id', uor.role_id::text))
        )
    )
"""


@FunctionTool
async def get_activity_carousel(
    wbs: str | None = None,
    activity_name: str | None = None,
    component_name: str | None = None,
    module_template_name: str | None = None,
    status: str | None = None,
    boq_name: str | None = None,
    limit: int = 6,
    offset: int = 0,
    *,
    tool_context: ToolContext = None,
) -> Dict[str, Any]:
    """Search activities and emit activity_carousel UI component for selection.

    Implements progressive WBS fallback:
    - Tier 1: Exact WBS match
    - Tier 2: Parent WBS levels (1.1.1.1.5.5 → 1.1.1.1.5 → 1.1.1.1 → 1.1)
    - Tier 3: Parent filters only (Module Template, Module, BOQ)

    Args:
        wbs: WBS code filter (partial match with progressive fallback)
        activity_name: Activity name filter (partial match)
        component_name: Module name filter (partial match)
        module_template_name: Template name filter (partial match)
        status: 'NOT_STARTED' | 'ON_GOING' | 'COMPLETED'
        boq_name: BOQ name filter (exact match on linked BOQ items)
        limit: Max results (default 6)
        offset: Pagination offset (default 0)

    Returns:
        {"status": "ui_rendered", "component": "activity_carousel", "count": N}
    """
    try:
        user_id, project_id = extract_context(tool_context)
    except ValueError as e:
        return {"status": "error", "message": str(e)}

    if not db_service.is_initialized:
        return {"status": "error", "message": "Database service not available."}

    # Validate and sanitize inputs
    wbs = validate_string_param("wbs", wbs)
    activity_name = validate_string_param("activity_name", activity_name)
    component_name = validate_string_param("component_name", component_name)
    module_template_name = validate_string_param("module_template_name", module_template_name)
    status = validate_string_param("status", status)
    boq_name = validate_string_param("boq_name", boq_name)
    limit, _ = validate_pagination(limit, 6, max_limit=100)
    if offset < 0:
        offset = 0

    # Build base conditions (all filters except WBS)
    conditions = []
    params = [user_id, project_id]
    param_counter = 3

    # Store original WBS for tracking and fallback
    original_wbs = wbs
    wbs_tier_used = None

    # Activity name filter
    if activity_name:
        conditions.append(f"AND t.name ILIKE ${param_counter}")
        params.append(f"%{activity_name}%")
        param_counter += 1

    # Component/Module filter
    if component_name:
        conditions.append(f"AND component.name ILIKE ${param_counter}")
        params.append(f"%{component_name}%")
        param_counter += 1

    # Module Template filter
    if module_template_name:
        conditions.append(f"AND template.name ILIKE ${param_counter}")
        params.append(f"%{module_template_name}%")
        param_counter += 1

    # Status filter
    if status:
        conditions.append(f"AND t.status = ${param_counter}")
        params.append(status)
        param_counter += 1

    # BOQ filter - add as EXISTS subquery
    boq_param_index = param_counter
    if boq_name:
        params.append(boq_name)
        boq_exists_clause = f"""
    AND EXISTS (
        SELECT 1
        FROM production.resource_allocation ra
        JOIN production.boq b ON b.id = ra.boq_item_id
        WHERE ra.task_id = t.id
        AND b.name = ${boq_param_index}
    )"""
        param_counter += 1
    else:
        boq_exists_clause = ""

    # Modify base SQL to include BOQ filter
    sql_with_boq = ACTIVITY_CAROUSEL_SQL + boq_exists_clause

    results = []
    total_count = 0

    # TIER 1: Exact WBS match + children (not substring)
    # "2.1.5.1.1" matches exactly OR children like "2.1.5.1.1.1"
    if wbs:
        wbs_conditions = conditions.copy()
        wbs_params = params.copy()
        wbs_conditions.append(f"AND (t.wbs = ${param_counter} OR t.wbs LIKE ${param_counter + 1})")
        wbs_params.append(wbs)
        wbs_params.append(f"{wbs}.%")

        sql_query = sql_with_boq + "\n".join(wbs_conditions) + f"\nORDER BY t.wbs\nLIMIT {limit} OFFSET {offset}"
        count_query = "SELECT COUNT(*) as total FROM (" + sql_with_boq.strip() + "\n".join(wbs_conditions) + ") AS count_table"

        try:
            count_result = await db_service.fetch_one(count_query, *wbs_params)
            total_count = count_result.get("total", 0) if count_result else 0

            if total_count > 0:
                results = await db_service.fetch_all(sql_query, *wbs_params)
                wbs_tier_used = "exact"
                logger.info(f"Activity carousel: Exact WBS match for '{wbs}', found {total_count}")
        except Exception as e:
            logger.error(f"Error in exact WBS match: {e}", exc_info=True)

    # TIER 2: Progressive WBS fallback
    if not results and wbs:
        fallback_levels = _get_wbs_fallback_levels(wbs)

        for fallback_wbs in fallback_levels:
            wbs_conditions = conditions.copy()
            wbs_params = params.copy()
            wbs_conditions.append(f"AND t.wbs ILIKE ${param_counter}")
            wbs_params.append(f"{fallback_wbs}%")  # No leading % for parent match

            sql_query = sql_with_boq + "\n".join(wbs_conditions) + f"\nORDER BY t.wbs\nLIMIT {limit} OFFSET {offset}"
            count_query = "SELECT COUNT(*) as total FROM (" + sql_with_boq.strip() + "\n".join(wbs_conditions) + ") AS count_table"

            try:
                count_result = await db_service.fetch_one(count_query, *wbs_params)
                total_count = count_result.get("total", 0) if count_result else 0

                if total_count > 0:
                    results = await db_service.fetch_all(sql_query, *wbs_params)
                    wbs_tier_used = f"parent_{fallback_wbs}"
                    logger.info(f"Activity carousel: WBS fallback to '{fallback_wbs}', original '{wbs}', found {total_count}")
                    break
            except Exception as e:
                logger.error(f"Error in WBS fallback {fallback_wbs}: {e}", exc_info=True)

    # TIER 3: Parent filters only (no WBS)
    if not results and (component_name or module_template_name):
        sql_query = sql_with_boq + "\n".join(conditions) + f"\nORDER BY t.wbs\nLIMIT {limit} OFFSET {offset}"
        count_query = "SELECT COUNT(*) as total FROM (" + sql_with_boq.strip() + "\n".join(conditions) + ") AS count_table"

        try:
            count_result = await db_service.fetch_one(count_query, *params)
            total_count = count_result.get("total", 0) if count_result else 0

            if total_count > 0:
                results = await db_service.fetch_all(sql_query, *params)
                wbs_tier_used = "parent_filters_only"
                logger.info(f"Activity carousel: Parent filters only for WBS '{wbs}', found {total_count}")
        except Exception as e:
            logger.error(f"Error in parent filter fallback: {e}", exc_info=True)

    # No WBS provided - just use other filters
    elif not wbs:
        sql_query = sql_with_boq + "\n".join(conditions) + f"\nORDER BY t.wbs\nLIMIT {limit} OFFSET {offset}"
        count_query = "SELECT COUNT(*) as total FROM (" + sql_with_boq.strip() + "\n".join(conditions) + ") AS count_table"

        try:
            count_result = await db_service.fetch_one(count_query, *params)
            total_count = count_result.get("total", 0) if count_result else 0

            if total_count > 0:
                results = await db_service.fetch_all(sql_query, *params)
        except Exception as e:
            logger.error(f"Error querying activities: {e}", exc_info=True)

    if not results:
        return {
            "status": "not_found",
            "message": "No activities found matching your criteria.",
        }

    # Build carousel payload
    activities = []
    for r in results:
        module_path = []
        if r.get("component_name"):
            module_path.append(r["component_name"])
        if r.get("template_name") and r["template_name"] != r.get("component_name"):
            module_path.insert(0, r["template_name"])

        activities.append({
            "id": str(r["id"]),
            "wbsNumber": r["wbs"],
            "description": r["name"],
            "modulePath": module_path,
        })

    # Dynamic header based on fallback tier
    if wbs_tier_used == "exact":
        header_text = "I found multiple matching activities. Select the correct one to continue so I can log consumption accurately."
    elif wbs_tier_used and wbs_tier_used.startswith("parent_"):
        if wbs_tier_used == "parent_filters_only":
            filter_parts = []
            if module_template_name:
                filter_parts.append(f"Module Template '{module_template_name}'")
            if component_name:
                filter_parts.append(f"Module '{component_name}'")
            if boq_name:
                filter_parts.append(f"BOQ '{boq_name}'")
            header_text = f"No exact match for WBS '{original_wbs}'. Showing activities from {', '.join(filter_parts)}."
        else:
            parent_wbs = wbs_tier_used.replace("parent_", "")
            header_text = f"No exact match for WBS '{original_wbs}'. Showing activities under parent WBS '{parent_wbs}'."
    else:
        header_text = "I found multiple matching activities. Select the correct one to continue so I can log consumption accurately."

    carousel_payload = {
        "activities": activities,
        "header_text": header_text,
        "show_reasoning": False,
        "total_count": total_count,
        "limit": limit,
        "offset": offset,
        "has_more": (offset + limit) < total_count,
        "remaining_count": max(0, total_count - offset - len(activities)),
    }

    emit_ui_component(
        tool_context,
        UIComponentType.ACTIVITY_CAROUSEL,
        carousel_payload,
        requires_response=True,
    )

    return {
        "status": "ui_rendered",
        "component": "activity_carousel",
        "count": len(activities),
        "total_count": total_count,
        "message": f"Found {total_count} activities using {wbs_tier_used or 'standard'} matching.",
    }
