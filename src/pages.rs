//! Page contents: Play, Edit voice, Effects, Settings.

use crate::app::{card, App, LearnTarget, Tab};
use crate::fm::is_carrier;
use crate::fx::{ChorusMode, DIVISIONS};
use crate::library::{Category, Source};
use crate::patch::*;
use crate::theme::{category_color, Theme, ThemeId};
use crate::widgets::{self, Vec2Size};
use egui::{self, vec2, Align, Layout, RichText, Slider};

fn pct(v: f32) -> String {
    format!("{:.0}%", v * 100.0)
}

fn small(t: &Theme, s: &str) -> RichText {
    RichText::new(s).size(crate::theme::fs(11.5)).color(t.text_dim)
}

// ================================================================== PLAY

pub fn play_page(app: &mut App, ui: &mut egui::Ui) {
    let t = app.theme;
    let wide = ui.available_width() > 820.0;
    card(ui, &t, "Performance", |ui| {
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing.x = 4.0;
            let p = &mut app.edit.play;
            widgets::knob(ui, &t, "Volume", &mut p.volume_db, -24.0, 12.0, 0.0, &|v| format!("{v:+.1} dB"), 58.0);
            widgets::knob(ui, &t, "Cutoff", &mut p.cutoff, 0.0, 1.0, 1.0, &|v| {
                let hz = crate::fx::cutoff_hz(v);
                if hz >= 1000.0 { format!("{:.1} kHz", hz / 1000.0) } else { format!("{hz:.0} Hz") }
            }, 58.0);
            widgets::knob(ui, &t, "Resonance", &mut p.resonance, 0.0, 1.0, 0.0, &pct, 58.0);
            widgets::knob(ui, &t, "Pan", &mut p.pan, -1.0, 1.0, 0.0, &|v| if v.abs() < 0.01 { "C".into() } else if v < 0.0 { format!("L{:.0}", -v * 100.0) } else { format!("R{:.0}", v * 100.0) }, 58.0);
            ui.add_space(14.0);
            ui.separator();
            ui.add_space(14.0);
            let r = widgets::knob(ui, &t, "Reverb", &mut p.reverb.amount, 0.0, 1.0, 0.0, &pct, 58.0);
            if r.changed() && p.reverb.amount > 0.0 {
                p.reverb.on = true;
            }
            let r = widgets::knob(ui, &t, "Chorus", &mut p.chorus.amount, 0.0, 1.0, 0.0, &pct, 58.0);
            if r.changed() && p.chorus.amount > 0.0 {
                p.chorus.on = true;
            }
            let r = widgets::knob(ui, &t, "Delay", &mut p.delay.amount, 0.0, 1.0, 0.0, &pct, 58.0);
            if r.changed() && p.delay.amount > 0.0 {
                p.delay.on = true;
            }
            ui.vertical(|ui| {
                ui.add_space(14.0);
                if ui.button("Effect settings…").clicked() {
                    app.tab = Tab::Effects;
                }
                ui.label(small(&t, "Saved with each sound"));
            });
        });
    });
    ui.add_space(10.0);

    let voice_card = |app: &mut App, ui: &mut egui::Ui| {
        card(ui, &t, "Voice", |ui| {
            let alg = (app.edit.voice.get(ALGORITHM) & 31) as usize;
            let (meters, enabled) = app.synth.lock().map(|s| (s.engine.op_meter, s.engine.op_enabled)).unwrap_or(([0.0; 6], [true; 6]));
            ui.horizontal(|ui| {
                if let Some(op) = widgets::algorithm_diagram(ui, &t, alg, &enabled, &meters, Vec2Size(220.0, 170.0)) {
                    app.selected_op = op;
                    app.tab = Tab::Edit;
                }
                ui.vertical(|ui| {
                    ui.label(small(&t, "Operators — click to switch on or off"));
                    for op in 0..6 {
                        ui.horizontal(|ui| {
                            let mut on = enabled[op];
                            let carrier = is_carrier(alg, op);
                            let label = RichText::new(format!("OP{} {}", op + 1, if carrier { "C" } else { "M" })).monospace().size(crate::theme::fs(12.0)).color(if on { t.text } else { t.text_faint });
                            if ui.toggle_value(&mut on, label).on_hover_text(if carrier { "Carrier: heard directly" } else { "Modulator: shapes the tone" }).changed() {
                                if let Ok(mut s) = app.synth.lock() {
                                    s.engine.op_enabled[op] = on;
                                }
                            }
                            let lvl = app.edit.voice.op(op, OP_LEVEL) as f32 / 99.0;
                            widgets::level_bar(ui, &t, lvl, 110.0, on);
                            ui.label(RichText::new(freq_text(&app.edit.voice, op)).monospace().size(crate::theme::fs(11.0)).color(t.text_faint));
                        });
                    }
                });
            });
            ui.add_space(4.0);
            let v = &app.edit.voice;
            let waves = ["Triangle", "Saw down", "Saw up", "Square", "Sine", "S&H"];
            ui.label(small(&t, &format!(
                "Algorithm {}  ·  Feedback {}  ·  LFO {} {}  ·  Transpose {:+}",
                alg + 1,
                v.get(FEEDBACK),
                waves[(v.get(LFO_WAVE) as usize).min(5)],
                v.get(LFO_SPEED),
                v.get(TRANSPOSE) as i32 - 24
            )));
        });
    };

    if wide {
        ui.columns(2, |cols| {
            voice_card(app, &mut cols[0]);
            sound_info_card(app, &mut cols[1]);
        });
    } else {
        voice_card(app, ui);
        ui.add_space(10.0);
        sound_info_card(app, ui);
    }
}

fn freq_text(v: &Voice, op: usize) -> String {
    let c = v.op(op, OP_COARSE) as f32;
    let f = v.op(op, OP_FINE) as f32;
    let det = v.op(op, OP_DETUNE) as i32 - 7;
    let d = if det != 0 { format!(" {det:+}") } else { String::new() };
    if v.op(op, OP_MODE) == 1 {
        let hz = 10f32.powf(((c as i32 & 3) as f32 * 100.0 + f) / 100.0);
        if hz >= 1000.0 { format!("{:.2} kHz{d}", hz / 1000.0) } else { format!("{hz:.1} Hz{d}") }
    } else {
        let r = if c == 0.0 { 0.5 } else { c } * (1.0 + f / 100.0);
        format!("×{r:.2}{d}")
    }
}

fn sound_info_card(app: &mut App, ui: &mut egui::Ui) {
    let t = app.theme;
    card(ui, &t, "About this sound", |ui| {
        let Some(id) = app.current else {
            ui.label(small(&t, "This sound has not been saved yet. Use Save as… to add it to your library."));
            return;
        };
        let folders = app.lib.folders.clone();
        let mut changed = false;
        let mut new_tag: Option<String> = None;
        let mut remove_tag: Option<usize> = None;
        let mut filter_tag: Option<String> = None;
        let mut rename = false;
        let mut new_folder = false;
        {
            let mut tag_input = std::mem::take(app.tag_input());
            let Some(p) = app.lib.get_mut(id) else { return };
            egui::Grid::new("info_grid").num_columns(2).spacing(vec2(10.0, 8.0)).show(ui, |ui| {
                ui.label(small(&t, "Name"));
                ui.horizontal(|ui| {
                    ui.label(RichText::new(&p.name).strong());
                    if ui.small_button("Rename…").clicked() {
                        rename = true;
                    }
                });
                ui.end_row();
                let raw = p.voice().name();
                if !raw.trim().is_empty() && raw.trim() != p.name {
                    ui.label(small(&t, "Bank name"));
                    ui.label(RichText::new(raw).monospace().color(t.text_faint)).on_hover_text("The name stored in the original bank. Exported banks keep up to 10 characters of the name shown above.");
                    ui.end_row();
                }
                ui.label(small(&t, "Source"));
                let src = match &p.source {
                    Source::Factory(b) => format!("Factory · {b} · slot {}", p.slot.map(|s| s + 1).unwrap_or(0)),
                    Source::Imported(b) => format!("Imported · {b} · slot {}", p.slot.map(|s| s + 1).unwrap_or(0)),
                    Source::User => "My sounds".into(),
                };
                ui.label(RichText::new(src).color(t.text_dim));
                ui.end_row();
                ui.label(small(&t, "Category"));
                egui::ComboBox::from_id_salt("info_cat").selected_text(RichText::new(p.category.label()).color(category_color(&t, p.category))).show_ui(ui, |ui| {
                    for c in Category::ALL {
                        if ui.selectable_value(&mut p.category, c, RichText::new(c.label()).color(category_color(&t, c))).changed() {
                            changed = true;
                        }
                    }
                });
                ui.end_row();
                ui.label(small(&t, "Rating"));
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 1.0;
                    for i in 1..=5u8 {
                        let filled = p.rating >= i;
                        if ui.add(egui::Label::new(RichText::new(if filled { "★" } else { "☆" }).size(crate::theme::fs(16.0)).color(if filled { t.star } else { t.text_faint })).sense(egui::Sense::click())).clicked() {
                            p.rating = if p.rating == i { 0 } else { i };
                            changed = true;
                        }
                    }
                    ui.add_space(10.0);
                    if ui.checkbox(&mut p.favorite, "Favourite").changed() {
                        changed = true;
                    }
                });
                ui.end_row();
                ui.label(small(&t, "Tags"));
                ui.horizontal_wrapped(|ui| {
                    ui.spacing_mut().item_spacing = vec2(4.0, 4.0);
                    for (i, tag) in p.tags.iter().enumerate() {
                        let r = ui.add(egui::Button::new(RichText::new(format!("#{tag}  ×")).size(crate::theme::fs(11.5))).corner_radius(10.0));
                        if r.clicked() {
                            remove_tag = Some(i);
                        }
                        if r.secondary_clicked() {
                            filter_tag = Some(tag.clone());
                        }
                    }
                    let input = &mut tag_input;
                    let r = ui.add(egui::TextEdit::singleline(input).hint_text("add tag").desired_width(90.0));
                    if r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) && !input.trim().is_empty() {
                        new_tag = Some(input.trim().trim_start_matches('#').to_string());
                        input.clear();
                        r.request_focus();
                    }
                });
                ui.end_row();
                ui.label(small(&t, "Folders"));
                ui.horizontal_wrapped(|ui| {
                    for f in &folders {
                        let mut inside = p.folders.contains(&f.id);
                        if ui.checkbox(&mut inside, &f.name).changed() {
                            if inside {
                                p.folders.push(f.id);
                            } else {
                                p.folders.retain(|x| *x != f.id);
                            }
                            changed = true;
                        }
                    }
                    if ui.small_button("+ New folder").clicked() {
                        new_folder = true;
                    }
                });
                ui.end_row();
                ui.label(small(&t, "Author"));
                if ui.add(egui::TextEdit::singleline(&mut p.author).hint_text("who made it").desired_width(200.0)).changed() {
                    changed = true;
                }
                ui.end_row();
                ui.label(small(&t, "Played"));
                ui.label(RichText::new(format!("{} time(s)", p.play_count)).color(t.text_dim));
                ui.end_row();
            });
            ui.add_space(4.0);
            ui.label(small(&t, "Notes"));
            if ui.add(egui::TextEdit::multiline(&mut p.notes).hint_text("Where you used it, playing tips…").desired_rows(3).desired_width(f32::INFINITY)).changed() {
                changed = true;
            }
            if let Some(tag) = new_tag {
                if !p.tags.iter().any(|x| x.eq_ignore_ascii_case(&tag)) {
                    p.tags.push(tag);
                }
                changed = true;
            }
            if let Some(i) = remove_tag {
                p.tags.remove(i);
                changed = true;
            }
            *app.tag_input() = tag_input;
        }
        if changed {
            app.mark_lib_dirty();
        }
        if rename {
            app.rename_dialog(id);
        }
        if new_folder {
            app.new_folder_dialog(Some(id));
        }
        if let Some(tag) = filter_tag {
            app.set_view_tag(tag);
        }
    });
}

// ================================================================== EDIT

fn pslider(ui: &mut egui::Ui, t: &Theme, label: &str, v: &mut u8, max: u8, width: f32) -> bool {
    let mut x = *v as i32;
    let mut changed = false;
    ui.horizontal(|ui| {
        ui.add_sized([86.0, 20.0], egui::Label::new(RichText::new(label).size(crate::theme::fs(11.5)).color(t.text_dim)));
        ui.spacing_mut().slider_width = width;
        if ui.add(Slider::new(&mut x, 0..=max as i32)).changed() {
            *v = x as u8;
            changed = true;
        }
    });
    changed
}

pub fn edit_page(app: &mut App, ui: &mut egui::Ui) {
    let t = app.theme;
    let alg = (app.edit.voice.get(ALGORITHM) & 31) as usize;
    let enabled = app.synth.lock().map(|s| s.engine.op_enabled).unwrap_or([true; 6]);
    let meters = app.synth.lock().map(|s| s.engine.op_meter).unwrap_or([0.0; 6]);

    ui.horizontal_top(|ui| {
        card(ui, &t, "Algorithm", |ui| {
            ui.set_width(250.0);
            ui.horizontal(|ui| {
                let v = &mut app.edit.voice;
                if ui.button("◀").clicked() {
                    v.0[ALGORITHM] = (v.0[ALGORITHM] + 31) % 32;
                }
                ui.label(RichText::new(format!("{:>2}", alg + 1)).monospace().size(crate::theme::fs(18.0)).color(t.lcd_text).background_color(t.lcd_bg));
                if ui.button("▶").clicked() {
                    v.0[ALGORITHM] = (v.0[ALGORITHM] + 1) % 32;
                }
                ui.label(small(&t, "Click a box to edit it"));
            });
            if let Some(op) = widgets::algorithm_diagram(ui, &t, alg, &enabled, &meters, Vec2Size(250.0, 170.0)) {
                app.selected_op = op;
            }
            let v = &mut app.edit.voice;
            pslider(ui, &t, "Feedback", &mut v.0[FEEDBACK], 7, 110.0);
            let mut tr = v.0[TRANSPOSE] as i32 - 24;
            ui.horizontal(|ui| {
                ui.add_sized([86.0, 20.0], egui::Label::new(small(&t, "Transpose")));
                ui.spacing_mut().slider_width = 110.0;
                if ui.add(Slider::new(&mut tr, -24..=24).suffix(" st")).changed() {
                    v.0[TRANSPOSE] = (tr + 24) as u8;
                }
            });
            let mut sync = v.0[OSC_SYNC] == 1;
            if ui.checkbox(&mut sync, "Restart oscillators on each note").changed() {
                v.0[OSC_SYNC] = sync as u8;
            }
            ui.horizontal(|ui| {
                ui.label(small(&t, "Voice name"));
                let mut nm = v.name();
                if ui.add(egui::TextEdit::singleline(&mut nm).char_limit(10).desired_width(110.0).font(egui::TextStyle::Monospace)).changed() {
                    v.set_name(&nm);
                }
            });
            ui.label(RichText::new("Up to 10 characters; stored inside .syx banks").size(crate::theme::fs(10.5)).color(t.text_faint));
        });

        ui.vertical(|ui| {
            card(ui, &t, "LFO", |ui| {
                let v = &mut app.edit.voice;
                let waves = ["Triangle", "Saw down", "Saw up", "Square", "Sine", "Sample & hold"];
                ui.horizontal(|ui| {
                    ui.add_sized([86.0, 20.0], egui::Label::new(small(&t, "Waveform")));
                    egui::ComboBox::from_id_salt("lfo_wave").selected_text(waves[(v.0[LFO_WAVE] as usize).min(5)]).show_ui(ui, |ui| {
                        for (i, w) in waves.iter().enumerate() {
                            ui.selectable_value(&mut v.0[LFO_WAVE], i as u8, *w);
                        }
                    });
                    let mut ks = v.0[LFO_SYNC] == 1;
                    if ui.checkbox(&mut ks, "Key sync").changed() {
                        v.0[LFO_SYNC] = ks as u8;
                    }
                });
                pslider(ui, &t, "Speed", &mut v.0[LFO_SPEED], 99, 120.0);
                pslider(ui, &t, "Delay", &mut v.0[LFO_DELAY], 99, 120.0);
                pslider(ui, &t, "Pitch depth", &mut v.0[LFO_PMD], 99, 120.0);
                pslider(ui, &t, "Amp depth", &mut v.0[LFO_AMD], 99, 120.0);
                pslider(ui, &t, "Pitch sens.", &mut v.0[PMS], 7, 120.0);
            });
            ui.add_space(8.0);
            card(ui, &t, "Pitch envelope", |ui| {
                let v = &mut app.edit.voice;
                let rates = [v.0[PEG_R1], v.0[PEG_R1 + 1], v.0[PEG_R1 + 2], v.0[PEG_R1 + 3]];
                let levels = [v.0[PEG_L1], v.0[PEG_L1 + 1], v.0[PEG_L1 + 2], v.0[PEG_L1 + 3]];
                widgets::envelope_graph(ui, &t, rates, levels, vec2(300.0, 50.0), None, t.accent2.gamma_multiply(1.4));
                egui::Grid::new("peg").num_columns(5).spacing(vec2(8.0, 3.0)).show(ui, |ui| {
                    ui.label("");
                    for i in 1..=4 {
                        ui.label(small(&t, &format!("Stage {i}")));
                    }
                    ui.end_row();
                    ui.label(small(&t, "Rate"));
                    for i in 0..4 {
                        let mut r = v.0[PEG_R1 + i] as i32;
                        if ui.add(egui::DragValue::new(&mut r).range(0..=99)).changed() {
                            v.0[PEG_R1 + i] = r as u8;
                        }
                    }
                    ui.end_row();
                    ui.label(small(&t, "Level"));
                    for i in 0..4 {
                        let mut l = v.0[PEG_L1 + i] as i32;
                        if ui.add(egui::DragValue::new(&mut l).range(0..=99)).changed() {
                            v.0[PEG_L1 + i] = l as u8;
                        }
                    }
                    ui.end_row();
                });
                ui.label(RichText::new("Level 50 = no pitch change").size(crate::theme::fs(10.5)).color(t.text_faint));
            });
        });
    });
    ui.add_space(10.0);

    // operator strip
    card(ui, &t, "Operators", |ui| {
        ui.horizontal(|ui| {
            for op in 0..6 {
                let sel = app.selected_op == op;
                let carrier = is_carrier(alg, op);
                let resp = egui::Frame::new()
                    .fill(if sel { t.raised } else { t.panel })
                    .stroke(egui::Stroke::new(1.0, if sel { t.accent } else { t.border }))
                    .corner_radius(5.0)
                    .inner_margin(8.0)
                    .show(ui, |ui| ui.vertical(|ui| {
                        ui.set_width(100.0);
                        ui.horizontal(|ui| {
                            ui.label(RichText::new(format!("OP {}", op + 1)).monospace().strong().color(if carrier { t.accent } else { t.text }));
                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                let mut on = enabled[op];
                                if ui.checkbox(&mut on, "").on_hover_text("Operator on/off (not saved)").changed() {
                                    if let Ok(mut s) = app.synth.lock() {
                                        s.engine.op_enabled[op] = on;
                                    }
                                }
                            });
                        });
                        ui.label(RichText::new(freq_text(&app.edit.voice, op)).monospace().size(crate::theme::fs(11.0)).color(t.text_dim));
                        widgets::level_bar(ui, &t, app.edit.voice.op(op, OP_LEVEL) as f32 / 99.0, 100.0, enabled[op]);
                        let v = &app.edit.voice;
                        let rates = [v.op(op, OP_R1), v.op(op, OP_R1 + 1), v.op(op, OP_R1 + 2), v.op(op, OP_R1 + 3)];
                        let levels = [v.op(op, OP_L1), v.op(op, OP_L1 + 1), v.op(op, OP_L1 + 2), v.op(op, OP_L1 + 3)];
                        widgets::envelope_graph(ui, &t, rates, levels, vec2(100.0, 34.0), Some(0), if carrier { t.accent } else { t.text_dim });
                    }))
                    .response
                    .interact(egui::Sense::click());
                if resp.clicked() {
                    app.selected_op = op;
                }
            }
        });
    });
    ui.add_space(10.0);
    operator_detail(app, ui);
}

fn operator_detail(app: &mut App, ui: &mut egui::Ui) {
    let t = app.theme;
    let op = app.selected_op;
    let title = format!("Operator {} details", op + 1);
    card(ui, &t, &title, |ui| {
        ui.horizontal(|ui| {
            if ui.button("Copy operator").clicked() {
                let mut buf = [0u8; OP_SIZE];
                for i in 0..OP_SIZE {
                    buf[i] = app.edit.voice.op(op, i);
                }
                app.op_clipboard = Some(buf);
                app.toast(format!("Copied operator {}", op + 1));
            }
            if ui.add_enabled(app.op_clipboard.is_some(), egui::Button::new("Paste operator")).clicked() {
                if let Some(buf) = app.op_clipboard {
                    for i in 0..OP_SIZE {
                        app.edit.voice.set_op(op, i, buf[i]);
                    }
                }
            }
            if ui.add_enabled(app.op_clipboard.is_some(), egui::Button::new("Paste envelope only")).clicked() {
                if let Some(buf) = app.op_clipboard {
                    for i in 0..8 {
                        app.edit.voice.set_op(op, OP_R1 + i, buf[OP_R1 + i]);
                    }
                }
            }
        });
        ui.add_space(6.0);
        let v = &mut app.edit.voice;
        let b = op_index(op, 0);
        ui.horizontal_top(|ui| {
            ui.vertical(|ui| {
                ui.set_width(270.0);
                ui.label(RichText::new("FREQUENCY & LEVEL").size(crate::theme::fs(10.5)).color(t.text_faint).strong());
                ui.horizontal(|ui| {
                    ui.add_sized([86.0, 20.0], egui::Label::new(small(&t, "Mode")));
                    ui.selectable_value(&mut v.0[b + OP_MODE], 0, "Ratio");
                    ui.selectable_value(&mut v.0[b + OP_MODE], 1, "Fixed");
                });
                pslider(ui, &t, "Coarse", &mut v.0[b + OP_COARSE], 31, 130.0);
                pslider(ui, &t, "Fine", &mut v.0[b + OP_FINE], 99, 130.0);
                let mut det = v.0[b + OP_DETUNE] as i32 - 7;
                ui.horizontal(|ui| {
                    ui.add_sized([86.0, 20.0], egui::Label::new(small(&t, "Detune")));
                    ui.spacing_mut().slider_width = 130.0;
                    if ui.add(Slider::new(&mut det, -7..=7)).changed() {
                        v.0[b + OP_DETUNE] = (det + 7) as u8;
                    }
                });
                ui.label(RichText::new(format!("= {}", freq_text(v, op))).monospace().color(t.accent));
                ui.add_space(4.0);
                pslider(ui, &t, "Output level", &mut v.0[b + OP_LEVEL], 99, 130.0);
                pslider(ui, &t, "Velocity sens.", &mut v.0[b + OP_VEL], 7, 130.0);
                pslider(ui, &t, "Amp mod sens.", &mut v.0[b + OP_AMS], 3, 130.0);
            });
            ui.separator();
            ui.vertical(|ui| {
                ui.set_width(300.0);
                ui.label(RichText::new("ENVELOPE").size(crate::theme::fs(10.5)).color(t.text_faint).strong());
                let rates = [v.0[b], v.0[b + 1], v.0[b + 2], v.0[b + 3]];
                let levels = [v.0[b + 4], v.0[b + 5], v.0[b + 6], v.0[b + 7]];
                widgets::envelope_graph(ui, &t, rates, levels, vec2(290.0, 80.0), Some(0), t.accent);
                egui::Grid::new("op_env").num_columns(5).spacing(vec2(8.0, 3.0)).show(ui, |ui| {
                    ui.label("");
                    for i in 1..=4 {
                        ui.label(small(&t, &format!("Stage {i}")));
                    }
                    ui.end_row();
                    ui.label(small(&t, "Rate"));
                    for i in 0..4 {
                        let mut x = v.0[b + OP_R1 + i] as i32;
                        if ui.add(egui::DragValue::new(&mut x).range(0..=99)).changed() {
                            v.0[b + OP_R1 + i] = x as u8;
                        }
                    }
                    ui.end_row();
                    ui.label(small(&t, "Level"));
                    for i in 0..4 {
                        let mut x = v.0[b + OP_L1 + i] as i32;
                        if ui.add(egui::DragValue::new(&mut x).range(0..=99)).changed() {
                            v.0[b + OP_L1 + i] = x as u8;
                        }
                    }
                    ui.end_row();
                });
                ui.label(RichText::new("Stage 3 holds while the key is down; stage 4 is the release.").size(crate::theme::fs(10.5)).color(t.text_faint));
                pslider(ui, &t, "Rate scaling", &mut v.0[b + OP_RATESCALE], 7, 130.0);
            });
            ui.separator();
            ui.vertical(|ui| {
                ui.set_width(270.0);
                ui.label(RichText::new("KEYBOARD SCALING").size(crate::theme::fs(10.5)).color(t.text_faint).strong());
                let names = ["C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B"];
                let bp = v.0[b + OP_BREAK] as i32 + 21;
                ui.horizontal(|ui| {
                    ui.add_sized([86.0, 20.0], egui::Label::new(small(&t, "Break point")));
                    let mut x = v.0[b + OP_BREAK] as i32;
                    ui.spacing_mut().slider_width = 110.0;
                    if ui.add(Slider::new(&mut x, 0..=99).show_value(false)).changed() {
                        v.0[b + OP_BREAK] = x as u8;
                    }
                    ui.label(RichText::new(format!("{}{}", names[(bp % 12) as usize], bp / 12 - 1)).monospace());
                });
                pslider(ui, &t, "Left depth", &mut v.0[b + OP_LDEPTH], 99, 130.0);
                pslider(ui, &t, "Right depth", &mut v.0[b + OP_RDEPTH], 99, 130.0);
                let curves = ["-Lin", "-Exp", "+Exp", "+Lin"];
                for (label, idx) in [("Left curve", OP_LCURVE), ("Right curve", OP_RCURVE)] {
                    ui.horizontal(|ui| {
                        ui.add_sized([86.0, 20.0], egui::Label::new(small(&t, label)));
                        for (i, c) in curves.iter().enumerate() {
                            ui.selectable_value(&mut v.0[b + idx], i as u8, *c);
                        }
                    });
                }
            });
        });
    });
}

// ================================================================== EFFECTS

/// Width of one effect card's contents: four knobs side by side.
const FX_KNOB: f32 = 50.0;
const FX_CARD_INNER: f32 = 4.0 * (FX_KNOB + 24.0 + 4.0);

pub fn effects_page(app: &mut App, ui: &mut egui::Ui) {
    let t = app.theme;
    ui.label(small(&t, "These settings are saved with each sound. Drag knobs up or down; double-click to reset."));
    ui.add_space(8.0);
    // Flow the cards into as many columns as fit the current window width and interface scale.
    let gap = 10.0;
    let card_w = FX_CARD_INNER + 26.0;
    let avail = ui.available_width();
    let cols = (((avail + gap) / (card_w + gap)).floor() as usize).clamp(1, 4);
    let cards = [0usize, 1, 2, 3];
    for row in cards.chunks(cols) {
        ui.horizontal_top(|ui| {
            ui.spacing_mut().item_spacing.x = gap;
            for &c in row {
                fx_card(app, ui, c);
            }
        });
        ui.add_space(gap);
    }
}

fn fx_card(app: &mut App, ui: &mut egui::Ui, which: usize) {
    let t = app.theme;
    let p = &mut app.edit.play;
    let k = FX_KNOB;
    let title = ["Filter & output", "Chorus", "Delay", "Reverb"][which];
    card(ui, &t, title, |ui| {
        ui.set_width(FX_CARD_INNER);
        ui.spacing_mut().item_spacing.x = 4.0;
        match which {
            0 => {
                ui.add_space(26.0);
                ui.horizontal(|ui| {
                    widgets::knob(ui, &t, "Cutoff", &mut p.cutoff, 0.0, 1.0, 1.0, &|v| {
                        let hz = crate::fx::cutoff_hz(v);
                        if hz >= 1000.0 { format!("{:.1} kHz", hz / 1000.0) } else { format!("{hz:.0} Hz") }
                    }, k);
                    widgets::knob(ui, &t, "Resonance", &mut p.resonance, 0.0, 1.0, 0.0, &pct, k);
                    widgets::knob(ui, &t, "Volume", &mut p.volume_db, -24.0, 12.0, 0.0, &|v| format!("{v:+.1} dB"), k);
                    widgets::knob(ui, &t, "Pan", &mut p.pan, -1.0, 1.0, 0.0, &|v| format!("{:+.0}", v * 100.0), k);
                });
            }
            1 => {
                ui.checkbox(&mut p.chorus.on, "On");
                ui.horizontal(|ui| {
                    widgets::knob(ui, &t, "Amount", &mut p.chorus.amount, 0.0, 1.0, 0.0, &pct, k);
                    widgets::knob(ui, &t, "Rate", &mut p.chorus.rate_hz, 0.05, 5.0, 0.5, &|v| format!("{v:.2} Hz"), k);
                    widgets::knob(ui, &t, "Depth", &mut p.chorus.depth_ms, 0.0, 5.0, 2.0, &|v| format!("{v:.1} ms"), k);
                    widgets::knob(ui, &t, "Stereo", &mut p.chorus.spread, 0.0, 1.0, 1.0, &pct, k);
                });
                ui.horizontal(|ui| {
                    ui.label(small(&t, "Mode"));
                    for m in ChorusMode::ALL {
                        ui.selectable_value(&mut p.chorus.mode, m, m.label());
                    }
                });
            }
            2 => {
                ui.horizontal(|ui| {
                    ui.checkbox(&mut p.delay.on, "On");
                    ui.checkbox(&mut p.delay.sync, "Sync to tempo");
                });
                ui.horizontal(|ui| {
                    widgets::knob(ui, &t, "Amount", &mut p.delay.amount, 0.0, 1.0, 0.0, &pct, k);
                    widgets::knob(ui, &t, "Feedback", &mut p.delay.feedback, 0.0, 0.9, 0.35, &pct, k);
                    widgets::knob(ui, &t, "Damping", &mut p.delay.damping, 0.0, 1.0, 0.3, &pct, k);
                });
                if p.delay.sync {
                    ui.horizontal(|ui| {
                        ui.label(small(&t, "Left"));
                        egui::ComboBox::from_id_salt("div_l").selected_text(DIVISIONS[p.delay.div_l.min(7)].0).width(60.0).show_ui(ui, |ui| {
                            for (i, (n, _)) in DIVISIONS.iter().enumerate() {
                                ui.selectable_value(&mut p.delay.div_l, i, *n);
                            }
                        });
                        ui.label(small(&t, "Right"));
                        egui::ComboBox::from_id_salt("div_r").selected_text(DIVISIONS[p.delay.div_r.min(7)].0).width(60.0).show_ui(ui, |ui| {
                            for (i, (n, _)) in DIVISIONS.iter().enumerate() {
                                ui.selectable_value(&mut p.delay.div_r, i, *n);
                            }
                        });
                    });
                    ui.horizontal(|ui| {
                        ui.label(small(&t, "Tempo"));
                        ui.add(egui::DragValue::new(&mut p.delay.bpm).range(20.0..=crate::fx::TEMPO_MAX_BPM).suffix(" BPM"));
                    });
                } else {
                    ui.horizontal(|ui| {
                        widgets::knob(ui, &t, "Left time", &mut p.delay.time_l_ms, 20.0, 1500.0, 500.0, &|v| format!("{v:.0} ms"), k);
                        widgets::knob(ui, &t, "Right time", &mut p.delay.time_r_ms, 20.0, 1500.0, 500.0, &|v| format!("{v:.0} ms"), k);
                    });
                }
            }
            _ => {
                ui.checkbox(&mut p.reverb.on, "On");
                ui.horizontal(|ui| {
                    widgets::knob(ui, &t, "Amount", &mut p.reverb.amount, 0.0, 1.0, 0.0, &pct, k);
                    widgets::knob(ui, &t, "Size", &mut p.reverb.size, 0.0, 1.0, 0.21, &pct, k);
                    widgets::knob(ui, &t, "Damping", &mut p.reverb.damping, 0.0, 1.0, 0.5, &pct, k);
                    widgets::knob(ui, &t, "Pre-delay", &mut p.reverb.predelay_ms, 0.0, crate::fx::PREDELAY_MAX_MS, 45.0, &|v| format!("{v:.0} ms"), k);
                });
            }
        }
    });
}

// ================================================================== SETTINGS

pub fn settings_page(app: &mut App, ui: &mut egui::Ui) {
    let t = app.theme;
    let mut changed = false;
    // two columns that share the page width, whatever the text size
    let col_w = ((ui.available_width() - 40.0) / 2.0).max(300.0);
    // sliders give way to labels and values when the text is larger
    ui.spacing_mut().slider_width = (col_w - crate::theme::fs(250.0)).clamp(60.0, 160.0);
    ui.horizontal_top(|ui| {
        ui.vertical(|ui| {
            ui.set_width(col_w);
            card(ui, &t, "Audio", |ui| {
                if app.plugin {
                    ui.label(small(&t, "Your host manages the audio device, sample rate, buffer size and MIDI inputs."));
                    egui::Grid::new("audio_plugin").num_columns(2).spacing(vec2(10.0, 8.0)).show(ui, |ui| {
                        ui.label(small(&t, "Master volume"));
                        if ui.add(Slider::new(&mut app.settings.master_db, -24.0..=6.0).suffix(" dB")).changed() {
                            changed = true;
                        }
                        ui.end_row();
                        ui.label(small(&t, "Master tune"));
                        if ui.add(Slider::new(&mut app.settings.master_tune, -100.0..=100.0).suffix(" cents")).changed() {
                            changed = true;
                        }
                        ui.end_row();
                    });
                    return;
                }
                let (dev, sr, err) = app.audio_status();
                egui::Grid::new("audio").num_columns(2).spacing(vec2(10.0, 8.0)).show(ui, |ui| {
                    ui.label(small(&t, "Output"));
                    let mut chosen: Option<Option<String>> = None;
                    egui::ComboBox::from_id_salt("audio_dev").selected_text(&dev).width(220.0).show_ui(ui, |ui| {
                        if ui.selectable_label(app.settings.audio_device.is_none(), "System default").clicked() {
                            chosen = Some(None);
                        }
                        for d in app.audio_devices.clone() {
                            if ui.selectable_label(app.settings.audio_device.as_deref() == Some(d.as_str()), &d).clicked() {
                                chosen = Some(Some(d));
                            }
                        }
                    });
                    if let Some(c) = chosen {
                        app.restart_audio(c);
                        changed = true;
                    }
                    ui.end_row();
                    ui.label(small(&t, "Sample rate"));
                    ui.label(if sr > 0 { format!("{sr} Hz") } else { "—".into() });
                    ui.end_row();
                    ui.label(small(&t, "Master volume"));
                    if ui.add(Slider::new(&mut app.settings.master_db, -24.0..=6.0).suffix(" dB")).changed() {
                        changed = true;
                    }
                    ui.end_row();
                    ui.label(small(&t, "Master tune"));
                    if ui.add(Slider::new(&mut app.settings.master_tune, -100.0..=100.0).suffix(" cents")).changed() {
                        changed = true;
                    }
                    ui.end_row();
                });
                if let Some(e) = err {
                    ui.label(RichText::new(format!("The audio device could not be used: {e}")).color(t.danger));
                }
                ui.horizontal(|ui| {
                    if ui.button("Play test tone").clicked() {
                        app.test_tone();
                    }
                    if ui.button("Refresh devices").clicked() {
                        app.audio_devices = crate::audio::output_devices();
                    }
                    if ui.button("Restart audio").clicked() {
                        let d = app.settings.audio_device.clone();
                        app.restart_audio(d);
                    }
                });
                ui.label(small(&t, "Master volume and tune are the same for every sound; Volume on the Play page is saved with each sound."));
            });
            ui.add_space(10.0);
            card(ui, &t, "Appearance", |ui| {
                ui.horizontal(|ui| {
                    for id in ThemeId::ALL {
                        let th = Theme::get(id);
                        let sel = app.settings.theme == id;
                        let resp = egui::Frame::new()
                            .fill(th.bg)
                            .stroke(egui::Stroke::new(if sel { 2.0 } else { 1.0 }, if sel { t.accent } else { t.border }))
                            .corner_radius(5.0)
                            .inner_margin(8.0)
                            .show(ui, |ui| ui.vertical(|ui| {
                                ui.set_width(92.0);
                                ui.horizontal(|ui| {
                                    for c in [th.accent, th.accent2, th.lcd_text, th.text] {
                                        let (r, _) = ui.allocate_exact_size(vec2(14.0, 14.0), egui::Sense::hover());
                                        ui.painter().rect_filled(r, 3.0, c);
                                    }
                                });
                                ui.label(RichText::new(id.label()).size(crate::theme::fs(12.0)).color(th.text));
                            }))
                            .response
                            .interact(egui::Sense::click());
                        if resp.clicked() {
                            app.settings.theme = id;
                            changed = true;
                        }
                    }
                });
                ui.horizontal(|ui| {
                    ui.label(small(&t, "Interface scale"));
                    let mut s = app.settings.ui_scale;
                    if ui.add(Slider::new(&mut s, crate::theme::UI_SCALE_MIN..=crate::theme::UI_SCALE_MAX).step_by(0.05)).drag_stopped() {
                        app.settings.ui_scale = s;
                        changed = true;
                    } else if !ui.ctx().is_using_pointer() && (s - app.settings.ui_scale).abs() > 0.001 {
                        app.settings.ui_scale = s;
                        changed = true;
                    }
                    if ui.small_button("Reset").clicked() {
                        app.settings.ui_scale = 1.0;
                        changed = true;
                    }
                });
                ui.horizontal(|ui| {
                    ui.label(small(&t, "Text size"));
                    let mut ts = app.settings.text_size;
                    let r = ui.add(Slider::new(&mut ts, 0.8..=1.4).step_by(0.05).custom_formatter(|v, _| format!("{:.0}%", v * 100.0)));
                    if r.changed() {
                        app.settings.text_size = ts;
                        changed = true;
                    }
                    if ui.small_button("Reset").clicked() {
                        app.settings.text_size = 1.0;
                        changed = true;
                    }
                });
                if ui.checkbox(&mut app.settings.text_grows, "Text grows a little as the window gets bigger").changed() {
                    changed = true;
                }
                ui.label(small(&t, "Headings, labels and small print keep their sizes relative to each other."));
                if ui.checkbox(&mut app.settings.show_keyboard, "Show on-screen keyboard").changed() {
                    changed = true;
                }
            });
        });
        ui.vertical(|ui| {
            ui.set_width(col_w);
            card(ui, &t, "MIDI", |ui| {
                if app.plugin {
                    ui.label(small(&t, "In the plugin, the host routes MIDI to Hexa FM."));
                    ui.horizontal(|ui| {
                        ui.label(small(&t, "Next / previous sound"));
                        for (label, target) in [("Next", LearnTarget::Next), ("Previous", LearnTarget::Prev)] {
                            let cc = if target == LearnTarget::Next { app.settings.next_cc } else { app.settings.prev_cc };
                            let learning = app.learn == Some(target);
                            let text = if learning { "Press a button…".to_string() } else { format!("{label}: {}", cc.map(|c| format!("CC {c}")).unwrap_or_else(|| "Learn".into())) };
                            if ui.selectable_label(learning, text).clicked() {
                                app.learn = if learning { None } else { Some(target) };
                            }
                        }
                    });
                    return;
                }
                egui::Grid::new("midi").num_columns(2).spacing(vec2(10.0, 8.0)).show(ui, |ui| {
                    ui.label(small(&t, "Input"));
                    let cur = app.midi_port_name().unwrap_or("None").to_string();
                    let mut pick: Option<Option<String>> = None;
                    egui::ComboBox::from_id_salt("midi_in").selected_text(&cur).width(220.0).show_ui(ui, |ui| {
                        if ui.selectable_label(cur == "None", "None").clicked() {
                            pick = Some(None);
                        }
                        for p in app.midi_ports.clone() {
                            if ui.selectable_label(cur == p, &p).clicked() {
                                pick = Some(Some(p));
                            }
                        }
                    });
                    match pick {
                        Some(Some(p)) => {
                            app.connect_midi(&p, true);
                            changed = true;
                        }
                        Some(None) => {
                            app.disconnect_midi();
                            changed = true;
                        }
                        None => {}
                    }
                    ui.end_row();
                    ui.label(small(&t, "Channel"));
                    let ch = app.settings.midi_channel;
                    egui::ComboBox::from_id_salt("midi_ch").selected_text(if ch == 0 { "All channels".to_string() } else { format!("Channel {ch}") }).show_ui(ui, |ui| {
                        if ui.selectable_value(&mut app.settings.midi_channel, 0, "All channels").changed() {
                            changed = true;
                        }
                        for c in 1..=16u8 {
                            if ui.selectable_value(&mut app.settings.midi_channel, c, format!("Channel {c}")).changed() {
                                changed = true;
                            }
                        }
                    });
                    ui.end_row();
                    for (label, target) in [("Next sound", LearnTarget::Next), ("Previous sound", LearnTarget::Prev)] {
                        ui.label(small(&t, label));
                        ui.horizontal(|ui| {
                            let cc = if target == LearnTarget::Next { app.settings.next_cc } else { app.settings.prev_cc };
                            ui.label(RichText::new(cc.map(|c| format!("CC {c}")).unwrap_or_else(|| "Not assigned".into())).monospace());
                            let learning = app.learn == Some(target);
                            if ui.selectable_label(learning, if learning { "Press a button…" } else { "Learn" }).clicked() {
                                app.learn = if learning { None } else { Some(target) };
                            }
                            if cc.is_some() && ui.small_button("Clear").clicked() {
                                if target == LearnTarget::Next {
                                    app.settings.next_cc = None;
                                } else {
                                    app.settings.prev_cc = None;
                                }
                                changed = true;
                            }
                        });
                        ui.end_row();
                    }
                });
                if ui.checkbox(&mut app.settings.program_change, "Program change picks a sound from the current list").changed() {
                    changed = true;
                }
                if ui.button("Refresh MIDI ports").clicked() {
                    app.midi_ports = crate::audio::midi_ports();
                }
            });
            ui.add_space(10.0);
            card(ui, &t, "Playing", |ui| {
                egui::Grid::new("ctl").num_columns(2).spacing(vec2(10.0, 8.0)).show(ui, |ui| {
                    ui.label(small(&t, "Pitch bend up"));
                    changed |= ui.add(Slider::new(&mut app.settings.bend_up, 0.0..=12.0).step_by(1.0).suffix(" st")).changed();
                    ui.end_row();
                    ui.label(small(&t, "Pitch bend down"));
                    changed |= ui.add(Slider::new(&mut app.settings.bend_down, 0.0..=12.0).step_by(1.0).suffix(" st")).changed();
                    ui.end_row();
                    ui.label(small(&t, "Mod wheel to vibrato"));
                    changed |= ui.add(Slider::new(&mut app.settings.wheel_pitch, 0.0..=1.0)).changed();
                    ui.end_row();
                    ui.label(small(&t, "Mod wheel to tremolo"));
                    changed |= ui.add(Slider::new(&mut app.settings.wheel_amp, 0.0..=1.0)).changed();
                    ui.end_row();
                    ui.label(small(&t, "Glide time"));
                    changed |= ui.add(Slider::new(&mut app.settings.portamento, 0.0..=2.0).suffix(" s")).changed();
                    ui.end_row();
                    ui.label(small(&t, "Mono"));
                    changed |= ui.checkbox(&mut app.settings.mono, "One note at a time").changed();
                    ui.end_row();
                });
                ui.label(small(&t, "Computer keys: A W S E D F T G Y H U J K play notes, Z / X change octave, Up / Down arrows change sound, Esc stops all notes."));
            });
            ui.add_space(10.0);
            card(ui, &t, "Library", |ui| {
                let total = app.lib.presets.iter().filter(|p| !p.trashed).count();
                ui.label(small(&t, &format!("{total} sounds · {} folders · stored in your Application Support folder", app.lib.folders.len())));
                ui.horizontal(|ui| {
                    if ui.button("Show library folder").clicked() {
                        app.open_data_folder();
                    }
                    if ui.button("Restore factory sounds").clicked() {
                        app.restore_factory();
                    }
                });
            });
            ui.add_space(10.0);
            card(ui, &t, "About", |ui| {
                ui.label(RichText::new("Hexa FM 0.1").strong());
                ui.label(small(&t, "A six-operator FM synthesizer with a sound library built for finding, sorting and keeping sounds."));
                ui.label(small(&t, "Open source under the GNU GPL v3 or later, provided without any warranty."));
                ui.label(small(&t, "A redesign of AV Dexed (by AV Vibes), which is based on DEXED 1.1.0. Thank you to Pascal Gauthier and everyone who created and maintains DEXED."));
                ui.label(small(&t, "Factory banks: Dexed 01 and SynprezFM 01–32, as shipped with DEXED and AV Dexed, plus the owner's included banks (dx1, ROM1A–ROM4B)."));
                ui.hyperlink_to("DEXED project", "https://asb2m10.github.io/dexed/");
            });
        });
    });
    if changed {
        app.apply_controller_settings();
        app.mark_lib_dirty();
    }
}
