use crate::ui::progress_message::{ProgressMessage, ProgressPhase};
use crate::ui::theme;
use crate::ui::Icons;
use indicatif::{HumanDuration, MultiProgress, ProgressBar};
use owo_colors::OwoColorize;
use std::thread;
use std::time::Duration;

pub struct ProgressManager {
    mp: MultiProgress,
    _handle: thread::JoinHandle<()>,
}

impl ProgressManager {
    pub fn new(total_files: usize) -> (Self, crossbeam::channel::Sender<ProgressMessage>) {
        let (tx, rx) = crossbeam::channel::unbounded::<ProgressMessage>();

        let mp = MultiProgress::new();
        
        // 1. Phased Progress Bar (Line 1)
        let pb = mp.add(ProgressBar::new(total_files as u64));
        let pb = if console::Term::stdout().is_term() {
            pb
        } else {
            ProgressBar::hidden()
        };

        pb.set_style(
            indicatif::ProgressStyle::with_template(
                "{prefix} {msg} [{bar:20.cyan/blue}] {percent}%"
            )
            .unwrap()
            .progress_chars("█▌░"),
        );

        // 2. Pre-allocate 3 status lines (Lines 2, 3, 4) to prevent flickering/jumping
        let mut status_lines = Vec::with_capacity(3);
        for i in 0..3 {
            let line = mp.add(ProgressBar::new_spinner());
            if i == 2 {
                line.set_style(indicatif::ProgressStyle::with_template("{spinner:.cyan/blue} {msg}").unwrap());
                if console::Term::stdout().is_term() {
                    line.enable_steady_tick(Duration::from_millis(100));
                }
            } else {
                line.set_style(indicatif::ProgressStyle::with_template("{msg}").unwrap());
            }
            if !console::Term::stdout().is_term() {
                line.set_draw_target(indicatif::ProgressDrawTarget::hidden());
            }
            status_lines.push(line);
        }

        let pb_clone = pb.clone();

        let handle = thread::spawn(move || {
            let mut active_files: Vec<String> = Vec::new();
            let mut current_phase = ProgressPhase::Parsing;
            let mut total_count = total_files;
            let mut processed_count = 0;

            for msg in rx {
                match msg {
                    ProgressMessage::Started { phase, total } => {
                        current_phase = phase;
                        total_count = total;
                        processed_count = 0;
                        active_files.clear();
                        
                        let phase_idx = match phase {
                            ProgressPhase::Parsing => 1,
                            ProgressPhase::Linking => 2,
                            ProgressPhase::Embedding => 3,
                            ProgressPhase::Semantic => 4,
                        };
                        
                        let phase_name = match phase {
                            ProgressPhase::Parsing => "Parsing Files",
                            ProgressPhase::Linking => "Linking Symbols",
                            ProgressPhase::Embedding => "Generating Embeddings",
                            ProgressPhase::Semantic => "Semantic Analysis",
                        };

                        pb_clone.set_prefix(format!("Phase {}:", phase_idx));
                        pb_clone.set_message(phase_name);
                        pb_clone.set_length(total as u64);
                        pb_clone.set_position(0);
                        
                        // Reset status lines immediately
                        for line in &status_lines {
                            line.set_message("");
                        }
                    }
                    ProgressMessage::Progress { phase, current, file } => {
                        if phase == current_phase {
                            if current > 0 {
                                processed_count = current;
                            } else {
                                processed_count += 1;
                            }
                            pb_clone.set_position(processed_count as u64);
                            
                            if let Some(f) = file {
                                active_files.push(f);
                                if active_files.len() > 2 {
                                    active_files.remove(0);
                                }
                            }

                            let icon = match current_phase {
                                ProgressPhase::Parsing => Icons::NEW,
                                ProgressPhase::Linking => Icons::LINK,
                                ProgressPhase::Embedding => Icons::BRAIN,
                                ProgressPhase::Semantic => Icons::BOLT,
                            };

                            for i in 0..2 {
                                if i < active_files.len() {
                                    status_lines[i].set_message(format!(
                                        " {} {} {}",
                                        Icons::TREE_BRANCH.style(theme().dim.clone()),
                                        icon.style(theme().info.clone()),
                                        active_files[i].style(theme().muted.clone())
                                    ));
                                } else {
                                    status_lines[i].set_message("");
                                }
                            }

                            if total_count > processed_count {
                                let remaining = total_count - processed_count;
                                status_lines[2].set_message(format!(
                                    " {} {} items remaining...",
                                    Icons::TREE_END.style(theme().dim.clone()),
                                    remaining.style(theme().dim.clone())
                                ));
                            } else {
                                status_lines[2].set_message(format!(
                                    " {} {} completing phase...",
                                    Icons::TREE_END.style(theme().dim.clone()),
                                    Icons::CHECK.style(theme().success.clone())
                                ));
                            }
                        }
                    }
                    ProgressMessage::Finished { phase: _ } => {
                        // Force 100% completion in UI
                        let len = pb_clone.length().unwrap_or(0);
                        pb_clone.set_position(len);
                        for line in &status_lines {
                            line.set_message("");
                        }
                    }
                    ProgressMessage::Exit => {
                        break;
                    }
                    _ => {}
                }
            }
            
            // Cleanup: Finish all to clear them if dropped
            for line in status_lines {
                line.finish_and_clear();
            }
            pb_clone.finish_and_clear();
        });

        (
            Self {
                mp,
                _handle: handle,
            },
            tx,
        )
    }

    pub fn clear(&self) {
        self.mp.clear().ok();
    }

    pub fn finish_with_summary(
        &self,
        duration: Duration,
        files: usize,
        symbols: usize,
        edges: usize,
    ) {
        self.clear();
        println!();
        println!(
            "{} {}",
            Icons::CHECK.style(theme().success.clone()),
            format!("Complete in {}", HumanDuration(duration)).style(theme().success.clone())
        );
        println!(
            "  {} {}  {} {}  {} {}",
            Icons::FILE.style(theme().info.clone()),
            files,
            Icons::PACKAGE.style(theme().info.clone()),
            symbols,
            Icons::LINK.style(theme().info.clone()),
            edges
        );
    }
}

pub struct Spinner {
    pb: ProgressBar,
}

impl Spinner {
    pub fn new(message: &str) -> Self {
        let pb = ProgressBar::new_spinner();
        pb.set_message(message.to_string());
        if console::Term::stdout().is_term() {
            pb.enable_steady_tick(Duration::from_millis(100));
        }
        Self { pb }
    }

    pub fn set_message(&self, msg: &str) {
        self.pb.set_message(msg.to_string());
    }

    pub fn finish_with_message(&self, msg: &str) {
        self.pb.finish_with_message(msg.to_string());
    }
}
