use crate::ui::progress_message::{ProgressMessage, ProgressPhase};
use crate::ui::theme;
use crate::ui::Icons;
use indicatif::{HumanDuration, MultiProgress, ProgressBar};
use owo_colors::OwoColorize;
use std::thread;
use std::time::Duration;

pub struct ProgressManager {
    mp: MultiProgress,
    pb: ProgressBar,
    _handle: thread::JoinHandle<()>,
}

impl ProgressManager {
    pub fn new(total_files: usize) -> (Self, crossbeam::channel::Sender<ProgressMessage>) {
        let (tx, rx) = crossbeam::channel::unbounded::<ProgressMessage>();

        let mp = MultiProgress::new();
        
        // Main progress bar for the current phase
        let pb = mp.add(ProgressBar::new(total_files as u64));
        let pb = if console::Term::stdout().is_term() {
            pb
        } else {
            ProgressBar::hidden()
        };

        // Template for the phase line: "Phase X: [Name] [███...] 82%"
        pb.set_style(
            indicatif::ProgressStyle::with_template(
                "{prefix} {msg} [{bar:20.cyan/blue}] {percent}%"
            )
            .unwrap()
            .progress_chars("█▌░"),
        );

        let pb_clone = pb.clone();
        let mp_clone = mp.clone();

        let handle = thread::spawn(move || {
            let mut active_files: Vec<String> = Vec::new();
            let mut current_phase = ProgressPhase::Parsing;
            let mut total_count = total_files;
            let mut processed_count = 0;

            // Lines for active files
            let mut file_bars: Vec<ProgressBar> = Vec::new();

            for msg in rx {
                match msg {
                    ProgressMessage::Started { phase, total } => {
                        current_phase = phase;
                        total_count = total;
                        
                        let phase_idx = match phase {
                            ProgressPhase::Parsing => 1,
                            ProgressPhase::Linking => 2,
                            ProgressPhase::Embedding => 3,
                            ProgressPhase::Semantic => 4,
                        };
                        
                        pb_clone.set_prefix(format!("Phase {}:", phase_idx));
                        pb_clone.set_message(match phase {
                            ProgressPhase::Parsing => "Parsing Files",
                            ProgressPhase::Linking => "Linking Symbols",
                            ProgressPhase::Embedding => "Generating Embeddings",
                            ProgressPhase::Semantic => "Semantic Analysis",
                        });
                        pb_clone.set_length(total as u64);
                        pb_clone.set_position(0);
                        processed_count = 0;
                        active_files.clear();
                        
                        // Clear old file bars
                        for bar in file_bars.drain(..) {
                            bar.finish_and_clear();
                        }
                    }
                    ProgressMessage::Progress { phase, current: _, file } => {
                        if phase == current_phase {
                            processed_count += 1;
                            pb_clone.set_position(processed_count as u64);
                            
                            if let Some(f) = file {
                                active_files.push(f);
                                if active_files.len() > 3 {
                                    active_files.remove(0);
                                }

                                // Update file lines
                                // For simplicity in this implementation, we'll recreate the bars if needed
                                // or just update existing ones. indicatif's MultiProgress can be tricky 
                                // with dynamic additions. We'll use a simpler approach of just setting messages.
                                
                                while file_bars.len() < active_files.len() {
                                    let new_bar = mp_clone.add(ProgressBar::new_spinner());
                                    new_bar.set_style(indicatif::ProgressStyle::with_template("{msg}").unwrap());
                                    file_bars.push(new_bar);
                                }

                                for (i, f_path) in active_files.iter().enumerate() {
                                    let is_last = i == active_files.len() - 1;
                                    let prefix = if is_last { Icons::TREE_END } else { Icons::TREE_BRANCH };
                                    let icon = match current_phase {
                                        ProgressPhase::Parsing => Icons::NEW,
                                        ProgressPhase::Linking => Icons::LINK,
                                        ProgressPhase::Embedding => Icons::BRAIN,
                                        _ => Icons::SPARKLE,
                                    };
                                    
                                    file_bars[i].set_message(format!(
                                        " {} {} {}",
                                        prefix.style(theme().dim.clone()),
                                        icon.style(theme().info.clone()),
                                        f_path.style(theme().muted.clone())
                                    ));
                                }
                                
                                // If we have remaining files, show the count on the last line
                                if total_count > processed_count {
                                    let remaining = total_count - processed_count;
                                    if let Some(_last_bar) = file_bars.last() {
                                        // If we have 3 files already, replace the last one's message to include remaining?
                                        // Actually, let's keep 2 files and 1 remaining line as per the user's mockup.
                                        if active_files.len() >= 3 {
                                             let f_path = &active_files[active_files.len()-2];
                                             file_bars[active_files.len()-2].set_message(format!(
                                                " {} {} {}",
                                                Icons::TREE_BRANCH.style(theme().dim.clone()),
                                                Icons::MOD.style(theme().warn.clone()), // Use MOD for variety like mockup
                                                f_path.style(theme().muted.clone())
                                            ));
                                            
                                            file_bars.last().unwrap().set_message(format!(
                                                " {} {} {} files remaining...",
                                                Icons::TREE_END.style(theme().dim.clone()),
                                                "🔄".style(theme().info.clone()),
                                                remaining.style(theme().dim.clone())
                                            ));
                                        }
                                    }
                                }
                            }
                        }
                    }
                    ProgressMessage::Exit => {
                        break;
                    }
                    _ => {}
                }
            }
            
            // Cleanup
            for bar in file_bars {
                bar.finish_and_clear();
            }
            pb_clone.finish_and_clear();
        });

        (
            Self {
                mp,
                pb,
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
