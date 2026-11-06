//! Hexa FM Setup for Windows: terms, component choice, install, uninstall.
//! Everything it installs is embedded in this one .exe.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use eframe::egui::{self, Color32, RichText};
use std::path::{Path, PathBuf};

const APP_EXE: &[u8] = include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/../target/x86_64-pc-windows-msvc/release/hexa-fm.exe"));
const PLUGIN_DLL: &[u8] = include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/../target/x86_64-pc-windows-msvc/release/hexa_fm_plugin.dll"));
const TERMS: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/../installer/TERMS.txt"));
const MANUAL: &[u8] = include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/../manual/Hexa FM Manual.pdf"));
const VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(PartialEq, Clone, Copy)]
enum Step {
    Welcome,
    Terms,
    Choose,
    Done,
}

struct Places {
    app_dir: PathBuf,
    vst3_dir: PathBuf,
    clap_dir: PathBuf,
    start_menu: PathBuf,
}

fn env_path(var: &str, fallback: &str) -> PathBuf {
    std::env::var_os(var).map(PathBuf::from).unwrap_or_else(|| PathBuf::from(fallback))
}

fn places(all_users: bool) -> Places {
    if all_users {
        let pf = env_path("ProgramFiles", r"C:\Program Files");
        let common = env_path("CommonProgramFiles", r"C:\Program Files\Common Files");
        let pd = env_path("ProgramData", r"C:\ProgramData");
        Places {
            app_dir: pf.join("Hexa FM"),
            vst3_dir: common.join("VST3"),
            clap_dir: common.join("CLAP"),
            start_menu: pd.join(r"Microsoft\Windows\Start Menu\Programs"),
        }
    } else {
        let local = dirs::data_local_dir().unwrap_or_else(|| env_path("LOCALAPPDATA", r"C:\Users\Public"));
        let roaming = dirs::data_dir().unwrap_or_else(|| local.clone());
        Places {
            app_dir: local.join(r"Programs\Hexa FM"),
            vst3_dir: local.join(r"Programs\Common\VST3"),
            clap_dir: local.join(r"Programs\Common\CLAP"),
            start_menu: roaming.join(r"Microsoft\Windows\Start Menu\Programs"),
        }
    }
}

fn write(path: &Path, bytes: &[u8], log: &mut Vec<String>) -> Result<(), String> {
    if let Some(d) = path.parent() {
        std::fs::create_dir_all(d).map_err(|e| format!("{}: {e}", d.display()))?;
    }
    std::fs::write(path, bytes).map_err(|e| format!("{}: {e}", path.display()))?;
    log.push(path.display().to_string());
    Ok(())
}

struct Choice {
    app: bool,
    vst3: bool,
    clap: bool,
    all_users: bool,
}

fn install(c: &Choice) -> Result<Vec<String>, String> {
    let p = places(c.all_users);
    let mut log = Vec::new();
    // the manual and terms always go with the app folder
    write(&p.app_dir.join("Hexa FM Manual.pdf"), MANUAL, &mut log)?;
    write(&p.app_dir.join("Terms and licence.txt"), TERMS.as_bytes(), &mut log)?;
    if c.app {
        let exe = p.app_dir.join("Hexa FM.exe");
        write(&exe, APP_EXE, &mut log)?;
        let url = format!("[InternetShortcut]\r\nURL=file:///{}\r\nIconFile={}\r\nIconIndex=0\r\n", exe.display().to_string().replace('\\', "/"), exe.display());
        let _ = write(&p.start_menu.join("Hexa FM.url"), url.as_bytes(), &mut log);
    }
    if c.vst3 {
        write(&p.vst3_dir.join(r"Hexa FM.vst3\Contents\x86_64-win\Hexa FM.vst3"), PLUGIN_DLL, &mut log)?;
    }
    if c.clap {
        write(&p.clap_dir.join("Hexa FM.clap"), PLUGIN_DLL, &mut log)?;
    }
    // uninstaller: a copy of this setup program, registered with Windows
    if let Ok(me) = std::env::current_exe() {
        let un = p.app_dir.join("Uninstall Hexa FM.exe");
        if std::fs::copy(&me, &un).is_ok() {
            log.push(un.display().to_string());
            let root = if c.all_users { "HKLM" } else { "HKCU" };
            let key = format!(r"{root}\Software\Microsoft\Windows\CurrentVersion\Uninstall\HexaFM");
            let scope = if c.all_users { "--all-users" } else { "--just-me" };
            let cmd = format!("\"{}\" --uninstall {scope}", un.display());
            for (name, val) in [("DisplayName", "Hexa FM"), ("DisplayVersion", VERSION), ("Publisher", "Hexa FM"), ("UninstallString", cmd.as_str())] {
                let _ = std::process::Command::new("reg").args(["add", &key, "/v", name, "/t", "REG_SZ", "/d", val, "/f"]).output();
            }
            let _ = std::process::Command::new("reg").args(["add", &key, "/v", "DisplayIcon", "/t", "REG_SZ", "/d", &p.app_dir.join("Hexa FM.exe").display().to_string(), "/f"]).output();
        }
    }
    Ok(log)
}

fn uninstall(all_users: bool) -> Vec<String> {
    let p = places(all_users);
    let mut done = Vec::new();
    for f in [
        p.vst3_dir.join("Hexa FM.vst3"),
        p.clap_dir.join("Hexa FM.clap"),
        p.start_menu.join("Hexa FM.url"),
        p.app_dir.join("Hexa FM.exe"),
        p.app_dir.join("Hexa FM Manual.pdf"),
        p.app_dir.join("Terms and licence.txt"),
    ] {
        let ok = if f.is_dir() { std::fs::remove_dir_all(&f).is_ok() } else { std::fs::remove_file(&f).is_ok() };
        if ok {
            done.push(f.display().to_string());
        }
    }
    let root = if all_users { "HKLM" } else { "HKCU" };
    let _ = std::process::Command::new("reg").args(["delete", &format!(r"{root}\Software\Microsoft\Windows\CurrentVersion\Uninstall\HexaFM"), "/f"]).output();
    // the uninstaller itself can't delete its running exe; Windows removes the folder on the next
    // cleanup once it is empty. Your sound library (in AppData\Roaming\Hexa FM) is kept.
    done
}

fn can_write(dir: &Path) -> bool {
    let probe = dir.join(".hexafm-write-test");
    let ok = std::fs::create_dir_all(dir).is_ok() && std::fs::write(&probe, b"x").is_ok();
    let _ = std::fs::remove_file(&probe);
    ok
}

/// Restart this setup with administrator rights (Windows asks the person first).
fn relaunch_elevated(args: &str) -> bool {
    let Ok(me) = std::env::current_exe() else { return false };
    let ps = format!("Start-Process -FilePath '{}' -ArgumentList '{}' -Verb RunAs", me.display().to_string().replace('\'', "''"), args);
    std::process::Command::new("powershell").args(["-NoProfile", "-WindowStyle", "Hidden", "-Command", &ps]).spawn().is_ok()
}

struct Setup {
    step: Step,
    accepted: bool,
    choice: Choice,
    result: Option<Result<Vec<String>, String>>,
    uninstall_mode: Option<bool>,
    uninstalled: Option<Vec<String>>,
}

impl eframe::App for Setup {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let accent = Color32::from_rgb(0x4a, 0x9e, 0x8f);
        egui::TopBottomPanel::top("title").exact_height(64.0).frame(egui::Frame::new().fill(Color32::from_rgb(0x16, 0x14, 0x12)).inner_margin(16)).show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label(RichText::new("HEXA FM").size(22.0).strong().color(Color32::from_rgb(0xeb, 0xe4, 0xd3)));
                ui.label(RichText::new(format!("Setup {VERSION}")).color(Color32::from_rgb(0xa7, 0x9e, 0x8d)));
            });
        });
        egui::CentralPanel::default().frame(egui::Frame::new().fill(Color32::from_rgb(0x1b, 0x19, 0x17)).inner_margin(18)).show(ctx, |ui| {
            if let Some(all) = self.uninstall_mode {
                ui.heading("Remove Hexa FM");
                ui.add_space(8.0);
                match &self.uninstalled {
                    None => {
                        ui.label("This removes the Hexa FM app, its VST3 and CLAP plugins, the manual and the Start menu entry. Your sound library and settings are kept.");
                        ui.add_space(12.0);
                        if ui.add(egui::Button::new(RichText::new("Remove").color(Color32::BLACK)).fill(accent)).clicked() {
                            self.uninstalled = Some(uninstall(all));
                        }
                    }
                    Some(done) => {
                        ui.label(format!("Removed {} item(s). You can close this window.", done.len()));
                        if ui.button("Close").clicked() {
                            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                        }
                    }
                }
                return;
            }
            match self.step {
                Step::Welcome => {
                    ui.heading("Welcome");
                    ui.add_space(6.0);
                    ui.label("This installs Hexa FM, a six-operator FM synthesizer with a sound library.");
                    ui.label("You can choose the standalone app and the VST3 and CLAP plugins for your music software.");
                    ui.add_space(6.0);
                    ui.label(RichText::new("Audio Units (AU) are a Mac-only format and the old VST2 format is no longer licensed by Steinberg, so neither is offered on Windows.").size(12.0).color(Color32::GRAY));
                    ui.add_space(16.0);
                    if ui.add(egui::Button::new(RichText::new("Next").color(Color32::BLACK)).fill(accent)).clicked() {
                        self.step = Step::Terms;
                    }
                }
                Step::Terms => {
                    ui.heading("Terms and licence");
                    ui.add_space(6.0);
                    egui::ScrollArea::vertical().max_height(250.0).show(ui, |ui| {
                        ui.label(RichText::new(TERMS).size(12.5));
                    });
                    ui.add_space(8.0);
                    ui.checkbox(&mut self.accepted, "I have read and accept the terms and the GNU GPL v3 licence");
                    ui.add_space(8.0);
                    ui.horizontal(|ui| {
                        if ui.button("Back").clicked() {
                            self.step = Step::Welcome;
                        }
                        if ui.add_enabled(self.accepted, egui::Button::new(RichText::new("Next").color(Color32::BLACK)).fill(accent)).clicked() {
                            self.step = Step::Choose;
                        }
                    });
                }
                Step::Choose => {
                    ui.heading("Choose what to install");
                    ui.add_space(6.0);
                    ui.checkbox(&mut self.choice.app, "Hexa FM app (standalone)");
                    ui.checkbox(&mut self.choice.vst3, "VST3 plugin");
                    ui.checkbox(&mut self.choice.clap, "CLAP plugin");
                    ui.add_space(10.0);
                    ui.label("Install for");
                    ui.radio_value(&mut self.choice.all_users, false, "Just me (no administrator rights needed)");
                    ui.radio_value(&mut self.choice.all_users, true, "Everyone on this computer (asks for administrator rights)");
                    let p = places(self.choice.all_users);
                    ui.add_space(6.0);
                    ui.label(RichText::new(format!("App: {}\nVST3: {}\nCLAP: {}", p.app_dir.display(), p.vst3_dir.display(), p.clap_dir.display())).size(11.5).color(Color32::GRAY));
                    ui.add_space(12.0);
                    ui.horizontal(|ui| {
                        if ui.button("Back").clicked() {
                            self.step = Step::Terms;
                        }
                        let any = self.choice.app || self.choice.vst3 || self.choice.clap;
                        if ui.add_enabled(any, egui::Button::new(RichText::new("Install").color(Color32::BLACK)).fill(accent)).clicked() {
                            if self.choice.all_users && !can_write(&p.app_dir) {
                                let args = format!("--install --all-users{}{}{}", if self.choice.app { " --app" } else { "" }, if self.choice.vst3 { " --vst3" } else { "" }, if self.choice.clap { " --clap" } else { "" });
                                if relaunch_elevated(&args) {
                                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                                } else {
                                    self.result = Some(Err("Administrator rights were not granted. Choose \"Just me\" or run Setup as administrator.".into()));
                                    self.step = Step::Done;
                                }
                            } else {
                                self.result = Some(install(&self.choice));
                                self.step = Step::Done;
                            }
                        }
                    });
                }
                Step::Done => {
                    match &self.result {
                        Some(Ok(files)) => {
                            ui.heading("Hexa FM is installed");
                            ui.add_space(6.0);
                            ui.label(format!("{} file(s) were installed. Restart your music software or rescan plugins to see Hexa FM.", files.len()));
                            let p = places(self.choice.all_users);
                            ui.add_space(10.0);
                            ui.horizontal(|ui| {
                                if self.choice.app && ui.button("Open Hexa FM").clicked() {
                                    let _ = std::process::Command::new(p.app_dir.join("Hexa FM.exe")).spawn();
                                }
                                if ui.button("Open the manual").clicked() {
                                    let _ = std::process::Command::new("explorer").arg(p.app_dir.join("Hexa FM Manual.pdf")).spawn();
                                }
                                if ui.button("Close").clicked() {
                                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                                }
                            });
                        }
                        Some(Err(e)) => {
                            ui.heading("Something went wrong");
                            ui.label(e.as_str());
                            if ui.button("Back").clicked() {
                                self.step = Step::Choose;
                            }
                        }
                        None => {}
                    }
                }
            }
        });
    }
}

fn main() -> eframe::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let has = |a: &str| args.iter().any(|x| x == a);
    let mut setup = Setup {
        step: Step::Welcome,
        accepted: false,
        choice: Choice { app: true, vst3: true, clap: true, all_users: false },
        result: None,
        uninstall_mode: None,
        uninstalled: None,
    };
    if has("--uninstall") {
        setup.uninstall_mode = Some(has("--all-users"));
    } else if has("--install") {
        // resumed with administrator rights after the person accepted the terms
        setup.choice = Choice { app: has("--app"), vst3: has("--vst3"), clap: has("--clap"), all_users: has("--all-users") };
        setup.accepted = true;
        setup.result = Some(install(&setup.choice));
        setup.step = Step::Done;
    }
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_title("Hexa FM Setup").with_inner_size([640.0, 500.0]).with_resizable(false),
        ..Default::default()
    };
    eframe::run_native("Hexa FM Setup", options, Box::new(move |_cc| Ok(Box::new(setup))))
}
