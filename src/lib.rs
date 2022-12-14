use std::{
    fs::File,
    io::{BufRead, BufReader},
    path::PathBuf,
    sync::{Arc, Condvar, Mutex},
};

use eframe::CreationContext;
use logs::Logs;
use ui_logs_linear::LinearLogsUi;
use ui_logs_tree::TreeLogsUi;
use ui_settings::SettingsUi;

pub mod logs;
mod ui_logs_linear;
mod ui_logs_tree;
mod ui_settings;

pub struct App {
    logs: Logs,
    cur_status: ProcessorStatus,

    settings: Settings,

    tab: Tab,
    tree_logs_ui: TreeLogsUi,
    linear_logs_ui: LinearLogsUi,
    #[allow(dead_code)]
    settings_ui: SettingsUi,

    task_sender: ProcessorTaskSender,
    status_receiver: ProcessorStatusReceiver,
    _processor_thread: std::thread::JoinHandle<()>,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
enum Tab {
    Settings,
    TreeLogs,
    LinearLogs,
}

#[derive(Debug, Clone)]
struct Settings {
    available_paths: Vec<PathBuf>,
    picked_path: Option<String>,
}

type ProcessorTaskSender = Arc<(Mutex<Option<ProcessorTask>>, Condvar)>;
type ProcessorTaskReceiver = ProcessorTaskSender;
type ProcessorStatusSender = Arc<Mutex<ProcessorStatus>>;
type ProcessorStatusReceiver = ProcessorStatusSender;

enum ProcessorTask {
    OpenLogs(PathBuf),
    Cancel,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
enum ProcessorStatus {
    #[default]
    NotStarted,
    IoFailed,
    Cancelled,
    Reading,
    Done,
}

fn run_processor(
    task_receiver: ProcessorTaskReceiver,
    status_sender: ProcessorStatusSender,
    logs: Logs,
) {
    'main: loop {
        let (lock, condvar) = &*task_receiver;
        let task = {
            let mut task = lock.lock().unwrap();
            if task.is_none() {
                task = condvar.wait(task).unwrap();
            }
            task.take().unwrap()
        };

        match task {
            ProcessorTask::Cancel => {
                // Do nothing, this is only relevant within the other tasks, now we're just clearing it out
            }
            ProcessorTask::OpenLogs(path) => {
                logs.clear();
                *status_sender.lock().unwrap() = ProcessorStatus::Reading;
                let file = match File::open(&path) {
                    Ok(file) => file,
                    Err(_) => {
                        *status_sender.lock().unwrap() = ProcessorStatus::IoFailed;
                        continue 'main;
                    }
                };
                let mut buf_read = BufReader::new(file);

                const LINE_COUNT_CHECKIN: usize = 1000;
                let mut lines_since_checkin = 0;
                let mut cur_line = String::new();

                // TODO: do this in more bulk to avoid lots of locking?
                while let Ok(_line_length) = buf_read.read_line(&mut cur_line) {
                    // First check if we've been ordered to do something else
                    lines_since_checkin += 1;
                    if lines_since_checkin > LINE_COUNT_CHECKIN
                        && task_receiver.0.lock().unwrap().is_some()
                    {
                        *status_sender.lock().unwrap() = ProcessorStatus::Cancelled;
                        continue 'main;
                    }
                    let trim_line = cur_line.trim();
                    if trim_line.is_empty() {
                        continue;
                    }
                    logs.add_json_message(trim_line);
                    cur_line.clear();
                }
                *status_sender.lock().unwrap() = ProcessorStatus::Done;
            }
        }
    }
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.poll_processor_state();
        self.update_ui(ctx);
    }
}

// Core State Updating
impl App {
    pub fn new(_cc: &CreationContext<'_>) -> Self {
        let logs = Logs::new();
        let task_sender = ProcessorTaskSender::default();
        let task_receiver = task_sender.clone();
        let status_sender = ProcessorStatusSender::default();
        let status_receiver = status_sender.clone();
        let logs_handle = logs.clone();

        // FIXME(WASM): this doesn't work in wasm, move to async?
        let _processor_thread = std::thread::spawn(move || {
            run_processor(task_receiver, status_sender, logs_handle);
        });

        Self {
            _processor_thread,
            logs,
            cur_status: ProcessorStatus::NotStarted,
            settings: Settings {
                available_paths: Vec::new(),
                picked_path: None,
            },
            tab: Tab::Settings,
            linear_logs_ui: LinearLogsUi::default(),
            tree_logs_ui: TreeLogsUi::default(),
            settings_ui: SettingsUi::default(),
            task_sender,
            status_receiver,
        }
    }
    fn poll_processor_state(&mut self) {
        // Fetch updates from processing thread
        self.cur_status = *self.status_receiver.lock().unwrap();
    }

    fn set_path(&mut self, idx: usize) {
        let path = self.settings.available_paths[idx].clone();
        self.settings.picked_path = Some(path.display().to_string());
        let (lock, condvar) = &*self.task_sender;
        let mut new_task = lock.lock().unwrap();
        *new_task = Some(ProcessorTask::OpenLogs(path));
        self.tab = Tab::TreeLogs;
        condvar.notify_one();
    }

    fn cancel_processing(&mut self) {
        let (lock, condvar) = &*self.task_sender;
        let mut new_task = lock.lock().unwrap();
        *new_task = Some(ProcessorTask::Cancel);
        condvar.notify_one();
    }
}

impl App {
    fn update_ui(&mut self, ctx: &egui::Context) {
        egui::TopBottomPanel::top("tabs").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.selectable_value(&mut self.tab, Tab::Settings, "settings");
                ui.selectable_value(&mut self.tab, Tab::LinearLogs, "linear logs");
                ui.selectable_value(&mut self.tab, Tab::TreeLogs, "tree logs");
            });
        });
        egui::CentralPanel::default().show(ctx, |ui| match self.tab {
            Tab::Settings => self.ui_settings(ui, ctx),
            Tab::LinearLogs => self.ui_logs_linear(ui, ctx),
            Tab::TreeLogs => self.ui_logs_tree(ui, ctx),
        });
    }
}

#[cfg(target_arch = "wasm32")]
use eframe::wasm_bindgen::{self, prelude::*};

/// This is the entry-point for all the web-assembly.
/// This is called once from the HTML.
/// It loads the app, installs some callbacks, then returns.
/// You can add more callbacks like this if you want to call in to your code.
#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
pub fn start(canvas_id: &str) -> Result<(), wasm_bindgen::JsValue> {
    // Make sure panics are logged using `console.error`.
    console_error_panic_hook::set_once();

    // Redirect tracing to console.log and friends:
    tracing_wasm::set_as_global_default();

    // let web_options = eframe::WebOptions::default();
    eframe::start_web(canvas_id, Box::new(|cc| Box::new(App::new(cc))))
}
