//! Read-only native composition companion; the editor owns the timeline and project.
use crate::{EffectcraftApp, i18n::stereo::tr};
use craft_stereo_window::{Picture, StereoWindow};
use effectcraft_engine::{
    project::ItemId,
    render::{Backend, CancellableFootage, RenderOpts, Renderer},
};
use effectcraft_render::stereo::{Budget, StereoRequest};
use effectcraft_time::Tick;
use serde_json::{Value, json};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

#[derive(Clone, Debug, PartialEq, Eq)]
struct Key {
    epoch: u64,
    revision: u64,
    comp: ItemId,
    time: Tick,
    width: u32,
    budget: [u32; 2],
    opts: u64,
    purge: u64,
    playing: bool,
}

impl Key {
    fn same_scene(&self, other: &Self) -> bool {
        Self { time: other.time, ..self.clone() } == *other
    }
    fn accepts(&self, completed: &Self) -> bool {
        self == completed || (self.playing && completed.playing && self.same_scene(completed))
    }
}

struct Done {
    key: Key,
    picture: Picture,
    summary: Value,
}
struct Job {
    key: Key,
    cancel: Arc<AtomicBool>,
    thread: JoinHandle<Result<Done, String>>,
}

#[derive(Default)]
pub struct View {
    pub options_open: bool,
    window: Option<StereoWindow>,
    opened: Option<Instant>,
    job: Option<Job>,
    shown_key: Option<Key>,
    shown: Option<Value>,
    awaiting_feedback: bool,
}

impl Drop for View {
    fn drop(&mut self) {
        if let Some(job) = self.job.take() {
            job.cancel.store(true, Ordering::Relaxed);
            if job.thread.join().is_err() {
                log::error!("the stereo composition worker stopped unexpectedly during close");
            }
        }
    }
}

fn pack(eyes: [effectcraft_render::Image; 2], aspect: (u32, u32), background: [f32; 3]) -> Result<Picture, String> {
    let [left, right] = eyes;
    let (w, h) = (left.width, left.height);
    if (right.width, right.height) != (w, h) {
        return Err(tr("The stereo views have different native dimensions.").into());
    }
    let row = (w as usize).checked_mul(4).ok_or_else(|| tr("The stereo view has invalid native dimensions.").to_string())?;
    let size = row.checked_mul(h as usize).and_then(|n| n.checked_mul(2)).ok_or_else(|| tr("The stereo view has invalid native dimensions.").to_string())?;
    let (left, right) = (left.to_rgba8_over(background), right.to_rgba8_over(background));
    if w == 0 || h == 0 || left.len() != size / 2 || right.len() != size / 2 {
        return Err(tr("The stereo view has invalid native dimensions.").into());
    }
    let mut bytes = Vec::with_capacity(size);
    for (a, b) in left.chunks_exact(row).zip(right.chunks_exact(row)) {
        bytes.extend_from_slice(a);
        bytes.extend_from_slice(b);
    }
    Picture::new(w as usize * 2, h as usize, bytes.into(), true)
        .and_then(|picture| picture.with_pixel_aspect_ratio(aspect.0, aspect.1))
        .map_err(|error| error.to_string())
}

fn pixel_aspect(value: f64) -> Result<(u32, u32), String> {
    use effectcraft_engine::project::limits::{MAX_PIXEL_ASPECT, MIN_PIXEL_ASPECT};
    if !value.is_finite() || !(MIN_PIXEL_ASPECT..=MAX_PIXEL_ASPECT).contains(&value) {
        return Err("the composition pixel aspect ratio is outside its declared bounds".into());
    }
    let mut numerator = (value * 100_000_000.0).round() as u32;
    let mut denominator = 100_000_000u32;
    let (mut a, mut b) = (numerator, denominator);
    while b != 0 {
        let remainder = a % b;
        a = b;
        b = remainder;
    }
    numerator /= a;
    denominator /= a;
    Ok((numerator, denominator))
}

impl View {
    pub fn inspect(&self) -> Value {
        let width = self.shown.as_ref().and_then(|shown| shown["size"][0].as_u64()).and_then(|width| u32::try_from(width).ok());
        let live = self.window.as_ref().and_then(|window| window.depth_limits()).and_then(|limits| width.map(|width| limits.native_pixels(width)));
        json!({"pending":self.job.is_some(),"shown":self.shown,"awaitingDepthFeedback":self.awaiting_feedback,"liveNativeBudget":live.map(|(sunk,popped)|[sunk,popped])})
    }

    fn close(&mut self) {
        if let Some(job) = &self.job {
            job.cancel.store(true, Ordering::Relaxed);
        }
        self.window = None;
        self.opened = None;
        self.shown = None;
        self.shown_key = None;
    }

    fn retire(&mut self) -> Result<bool, String> {
        if self.job.as_ref().is_some_and(|job| !job.thread.is_finished()) {
            return Ok(false);
        }
        if let Some(job) = self.job.take() {
            let Job { thread, .. } = job;
            let result = thread.join().map_err(|_| "the stereo composition worker stopped unexpectedly".to_string())?;
            if !job.cancel.load(Ordering::Relaxed) {
                result?;
            }
        }
        Ok(true)
    }

    fn update(&mut self, app: &mut EffectcraftApp, ctx: &egui::Context) -> Result<(), String> {
        if !app.ui.stereo.enabled {
            self.close();
            if !self.retire()? {
                ctx.request_repaint_after(Duration::from_millis(16));
            }
            return Ok(());
        }
        let comp = app.session.active_comp_id().ok_or_else(|| tr("Open a composition first.").to_string())?;
        let document = app.session.project.comp(comp).ok_or_else(|| "the stereo composition was closed".to_string())?;
        let width = app.ui.stereo.width.unwrap_or(document.width);
        let aspect = pixel_aspect(document.pixel_aspect)?;
        let background = document.background;
        let time = app.session.time();
        let options = RenderOpts {
            backend: Backend::Cpu,
            scale: 1.0,
            motion_blur: true,
            guides: false,
            draft: false,
            view: None,
            roi: None,
            nested_switches: app.session.prefs.general.switches_affect_nested_comps,
            draft_shadows: app.session.prefs.three_d.realtime_shadows,
            ..Default::default()
        };
        if self.window.is_none() {
            self.window = Some(StereoWindow::open(tr("Stereo View"), "ai.storyteller.effectcraft.StereoView").map_err(|error| error.to_string())?);
            self.opened = None;
        }
        let window = self.window.as_ref().ok_or_else(|| "the stereo window is unavailable".to_string())?;
        if !window.is_open() {
            app.ui.stereo.enabled = false;
            self.close();
            return Ok(());
        }
        let limits = window.depth_limits().map(|limits| limits.native_pixels(width));
        let waiting = limits.is_none();
        let waiting_message = tr("Waiting for stereo depth feedback from the desktop.");
        if waiting && !self.awaiting_feedback {
            app.ui.status = waiting_message.into();
        }
        if !waiting && self.awaiting_feedback && app.ui.status == waiting_message {
            app.ui.status.clear();
        }
        self.awaiting_feedback = waiting;
        if limits.is_none() && self.opened.is_some_and(|opened| opened.elapsed() > Duration::from_secs(3)) {
            return Err(tr("The desktop did not provide stereo depth feedback. Open Display Settings and enable stereo support.").into());
        }
        let (sunk, popped) = limits.unwrap_or((0.0, 0.0));
        let budget = Budget { sunk: sunk as f32, popped: popped as f32 };
        let key = Key {
            epoch: app.session.script_ui.project_epoch,
            revision: app.session.revision,
            comp,
            time,
            width,
            budget: [budget.sunk.to_bits(), budget.popped.to_bits()],
            opts: crate::frames::opts_hash(&options),
            purge: app.purges,
            playing: app.playback.playing,
        };
        if let Some(job) = &self.job
            && !key.accepts(&job.key)
        {
            job.cancel.store(true, Ordering::Relaxed);
        }
        if self.job.as_ref().is_some_and(|job| job.thread.is_finished()) {
            let job = self.job.take().ok_or_else(|| "the completed stereo worker was lost".to_string())?;
            let cancelled = job.cancel.load(Ordering::Relaxed);
            let Job { thread, .. } = job;
            let result = thread.join().map_err(|_| "the stereo composition worker stopped unexpectedly".to_string())?;
            if !cancelled {
                let done = result?;
                if key.accepts(&done.key) {
                    if !window.show(done.picture) {
                        return Err("the stereo window closed before its complete pair was shown".into());
                    }
                    self.shown = Some(done.summary);
                    self.shown_key = Some(done.key);
                    if self.opened.is_none() {
                        self.opened = Some(Instant::now());
                    }
                }
            }
        }
        if self.job.is_none() && self.shown_key.as_ref() != Some(&key) {
            let (project, footage, expr) = (app.session.project.clone(), app.session.footage.clone(), app.session.expr.clone());
            let cancel = Arc::new(AtomicBool::new(false));
            let cancelled = cancel.clone();
            let wake = ctx.clone();
            let job_key = key.clone();
            let thread=std::thread::Builder::new().name("EffectCraft stereo composition".into()).spawn(move|| {
                let source=CancellableFootage { inner:footage.as_ref(),cancel:cancelled.clone() };
                let mut renderer=Renderer::new(&project,&source,options);
                renderer.expr=expr.as_deref();
                let started=Instant::now();
                let result=renderer.stereo_frame(comp,time,StereoRequest{width,budget}).and_then(|frame| {
                    let size=[frame.eyes[0].width,frame.eyes[0].height];
                    let summary=json!({"composition":comp.0,"revision":job_key.revision,"projectEpoch":job_key.epoch,"time":time.seconds(),"size":size,"budget":[budget.sunk,budget.popped],"depths":frame.depths,"resourceQueries":frame.resource_queries,"samples":frame.samples,"sameRootSamples":frame.samples[0]==frame.samples[1],"backend":"CPU","renderMs":started.elapsed().as_secs_f64()*1000.0,"pixelAspect":[aspect.0,aspect.1]});
                    Ok(Done{key:job_key,picture:pack(frame.eyes,aspect,background)?,summary})
                });
                wake.request_repaint();result
            }).map_err(|error|format!("could not start the stereo composition worker: {error}"))?;
            self.job = Some(Job { key, cancel, thread });
        }
        ctx.request_repaint_after(Duration::from_millis(16));
        Ok(())
    }

    fn settings(&mut self, app: &mut EffectcraftApp, ctx: &egui::Context) {
        if !self.options_open {
            return;
        }
        let mut open = true;
        egui::Window::new(tr("Render resolution per eye")).id(egui::Id::unique("stereo-resolution")).open(&mut open).show(ctx, |ui| {
            ui.label(tr("The document stage size preserves its original sampling; the desktop scales once."));
            if let Some(comp) = app.session.active_comp() {
                let mut width = app.ui.stereo.width.unwrap_or(comp.width);
                ui.horizontal(|ui| {
                    ui.label(tr("Width"));
                    let response = ui.add(egui::DragValue::new(&mut width).range(1..=u32::MAX));
                    app.auto.add_response("stereo.resolution.width", &response, tr("Width"));
                    if response.changed() {
                        app.ui.stereo.width = Some(width);
                    }
                    let height = (f64::from(width) * f64::from(comp.height) / f64::from(comp.width)).round();
                    ui.label(format!("{}: {height}", tr("Height")));
                });
                let response = ui.button(tr("Use document stage size"));
                app.auto.add_response("stereo.resolution.document", &response, tr("Use document stage size"));
                if response.clicked() {
                    app.ui.stereo.width = None;
                }
            }
        });
        self.options_open = open;
    }
}

pub fn tick(app: &mut EffectcraftApp, ctx: &egui::Context) {
    if app.stereo_view.is_none() && !app.ui.stereo.enabled {
        return;
    }
    let mut view = app.stereo_view.take().unwrap_or_default();
    if let Err(error) = view.update(app, ctx) {
        view.close();
        app.ui.stereo.enabled = false;
        app.ui.status = format!("{}: {error}", tr("Could not show the stereo view"));
    }
    if app.ui.stereo.enabled || view.options_open || view.job.is_some() {
        app.stereo_view = Some(view);
    }
}

pub fn settings(app: &mut EffectcraftApp, ctx: &egui::Context) {
    if let Some(mut view) = app.stereo_view.take() {
        view.settings(app, ctx);
        app.stereo_view = Some(view);
    }
}

pub fn display_settings() -> Result<(), String> {
    std::process::Command::new("kcmshell6")
        .arg("kcm_kscreen")
        .spawn()
        .map(|_| ())
        .map_err(|error| format!("{}: {error}", tr("Could not open Display Settings")))
}

#[cfg(test)]
mod tests {
    use super::*;
    fn key() -> Key {
        Key {
            epoch: 1,
            revision: 2,
            comp: ItemId(3),
            time: Tick::ZERO,
            width: 640,
            budget: [8.0f32.to_bits(), 12.0f32.to_bits()],
            opts: 4,
            purge: 0,
            playing: false,
        }
    }
    #[test]
    fn a_paused_seek_and_a_changed_world_refuse_obsolete_pairs() {
        let before = key();
        for after in [
            Key { time: Tick::from_seconds_f64(1.0), ..key() },
            Key { revision: 3, ..key() },
            Key { epoch: 2, ..key() },
            Key { width: 320, ..key() },
            Key { budget: [0, 0], ..key() },
            Key { purge: 1, ..key() },
        ] {
            assert!(!after.accepts(&before));
        }
        assert!(before.accepts(&before));
    }
    #[test]
    fn playback_keeps_completed_frame_pairs_but_never_crosses_scene_state() {
        let before = Key { playing: true, ..key() };
        let after = Key { time: Tick::from_seconds_f64(1.0), ..before.clone() };
        assert!(after.accepts(&before));
        assert!(!Key { revision: 3, ..after.clone() }.accepts(&before));
        assert!(!Key { playing: false, ..after }.accepts(&before));
    }
    #[test]
    fn the_stereo_companion_keeps_the_ordinary_document_background() {
        let mut session = effectcraft_engine::Session::default();
        session.execute("comp.new", json!({"width":2,"height":1,"background":"#805020"})).unwrap();
        let comp = session.active_comp_id().unwrap();
        let image = session.render(comp, Tick::ZERO, RenderOpts { backend: Backend::Cpu, ..Default::default() });
        let (_, _, ordinary) = session.render_rgba8(comp, Tick::ZERO, 0).unwrap();
        let row = image.width as usize * 4;
        let picture = pack([image.clone(), image], (1, 1), session.active_comp().unwrap().background).unwrap();
        let expected: Vec<u8> = ordinary.chunks_exact(row).flat_map(|pixels| pixels.iter().chain(pixels)).copied().collect();
        assert_eq!(picture.rgba.as_ref(), expected.as_slice());
    }
    #[test]
    fn packing_retains_each_real_eye_row_without_resampling() {
        let mut left = effectcraft_render::Image::new(3, 2);
        let mut right = effectcraft_render::Image::new(3, 2);
        left.data.fill([1.0, 0.0, 0.0, 1.0]);
        right.data.fill([0.0, 0.0, 1.0, 1.0]);
        let picture = pack([left, right], (1, 1), [0.0; 3]).unwrap();
        assert_eq!((picture.width, picture.height), (6, 2));
        for row in picture.rgba.as_chunks::<24>().0 {
            assert_eq!(&row[..4], &[255, 0, 0, 255]);
            assert_eq!(&row[12..16], &[0, 0, 255, 255]);
        }
    }
}

#[cfg(test)]
mod aspect_tests {
    use super::pixel_aspect;
    #[test]
    fn native_pixel_aspect_is_declared_without_resizing_samples() {
        for value in [0.1, 10.0, 1.0, 10.0 / 11.0, 40.0 / 33.0, 1.2121] {
            let (n, d) = pixel_aspect(value).unwrap();
            assert!((f64::from(n) / f64::from(d) - value).abs() <= 5.1e-9);
        }
        assert_eq!(pixel_aspect(1.0).unwrap(), (1, 1));
        for value in [0.0, -1.0, f64::NAN, f64::INFINITY, 100.0] {
            assert!(pixel_aspect(value).is_err());
        }
    }
}
