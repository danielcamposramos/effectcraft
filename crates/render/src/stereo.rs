//! Complete CPU eye pairs of one immutable composition frame.
mod frozen_footage;
use frozen_footage::FrozenFootage;
use std::sync::Mutex;

use crate::three_d::camera::{CameraState, NEAR, parent_world};
use crate::three_d::stereo_camera::{ViewCamera, camera_views};
use crate::{Backend, EvalCtx, Image, ItemId, RenderOpts, Renderer, Tick};
pub use craft_stereo_camera::Budget;
use effectcraft_geom::{Mat4, Vec3};

#[derive(Clone, Copy, Debug)]
pub struct StereoRequest {
    /// Per-eye width; the document aspect ratio determines height.
    pub width: u32,
    pub budget: Budget,
}

#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize)]
pub struct StereoSample {
    pub time: Tick,
    pub central: CameraState,
    pub zero_depth: f64,
    pub baseline: f64,
}

pub struct StereoFrame {
    pub eyes: [Image; 2],
    /// Unique evaluated root cameras, in matching order for each eye.
    pub samples: [Vec<StereoSample>; 2],
    /// Camera-depth / zero-plane-depth bounds, over all root shutter samples.
    pub depths: [f64; 2],
    /// Frozen query counts: float frames, byte frames, vectors, models, channels and audio.
    /// Unavailable results are retained too, so a pending input cannot resolve between eyes.
    pub resource_queries: [usize; 6],
}

struct Observation {
    error: Option<String>,
    near: f64,
    far: f64,
    infinity: bool,
    samples: Vec<StereoSample>,
}

pub(crate) struct Pass {
    comp: ItemId,
    eye: Option<usize>,
    request: StereoRequest,
    depths: [f64; 2],
    infinity: bool,
    state: Mutex<Observation>,
}

impl Pass {
    fn new(comp: ItemId, eye: Option<usize>, request: StereoRequest, depths: [f64; 2], infinity: bool) -> Self {
        Self {
            comp,
            eye,
            request,
            depths,
            infinity,
            state: Mutex::new(Observation { error: None, near: f64::INFINITY, far: 0.0, infinity: false, samples: vec![] }),
        }
    }

    pub(crate) fn fail(&self, error: impl Into<String>) {
        if let Ok(mut state) = self.state.lock() {
            state.error.get_or_insert_with(|| error.into());
        }
    }

    fn finish(&self) -> Result<std::sync::MutexGuard<'_, Observation>, String> {
        let state = self.state.lock().map_err(|_| "stereo scene observation was interrupted".to_string())?;
        if let Some(error) = &state.error {
            return Err(error.clone());
        }
        Ok(state)
    }
}

fn zero_depth(ctx: &EvalCtx, frame: Tick, central: CameraState, held: bool) -> f64 {
    if !held
        && let Some(camera) = ctx.comp.active_camera(frame)
        && crate::three_d::camera::is_two_node(camera)
        && let Some(transform) = camera.transform()
        && transform.get("poi").is_some()
    {
        let poi = ctx.v3(camera, transform, "poi", [ctx.comp.width as f64 / 2.0, ctx.comp.height as f64 / 2.0, 0.0]);
        return central.depth(parent_world(ctx, camera).apply(Vec3::from(poi)));
    }
    if central.ortho { central.depth(Vec3::from([ctx.comp.width as f64 / 2.0, ctx.comp.height as f64 / 2.0, 0.0])) } else { central.zoom }
}

impl Renderer<'_> {
    pub(crate) fn eye_camera(&self, ctx: &EvalCtx, frame: Tick, central: CameraState) -> ViewCamera {
        let Some(pass) = self.stereo.filter(|pass| ctx.comp_id == pass.comp) else { return ViewCamera::mono(central) };
        let zero = zero_depth(ctx, frame, central, self.opts.view.is_some());
        let budget = if pass.eye.is_some() { pass.request.budget } else { Budget { popped: 0.0, sunk: 0.0 } };
        let depths = pass.depths.map(|ratio| ratio * zero);
        match camera_views(central, [ctx.comp.width as f64, ctx.comp.height as f64], zero, depths, pass.request.width, budget, pass.infinity) {
            Ok(pair) => {
                if let Ok(mut state) = pass.state.lock() {
                    let sample = StereoSample { time: ctx.time, central, zero_depth: zero, baseline: pair.baseline };
                    if !state.samples.contains(&sample) {
                        if state.samples.len() >= 4096 {
                            state.error = Some("stereo frame exceeds the root camera sample limit".into());
                        } else {
                            state.samples.push(sample);
                        }
                    }
                }
                pair.eyes[pass.eye.unwrap_or(0)]
            }
            Err(error) => {
                pass.fail(error);
                ViewCamera::mono(central)
            }
        }
    }

    /// Observe actual world geometry before viewport culling, across every root shutter sample.
    pub(crate) fn stereo_points(&self, ctx: &EvalCtx, camera: &ViewCamera, points: impl IntoIterator<Item = Vec3>) {
        let Some(pass) = self.stereo_owner(ctx).filter(|pass| pass.eye.is_none()) else { return };
        let root = self.collapse3d.map_or(*ctx, |collapsed| collapsed.parent);
        let frame = self.collapse3d.map_or(ctx.time, |collapsed| collapsed.frame.time);
        let held = self.collapse3d.map_or(self.opts.view.is_some(), |collapsed| collapsed.view.is_some());
        let zero = zero_depth(&root, frame, **camera, held);
        let (mut near, mut far) = (f64::INFINITY, f64::NEG_INFINITY);
        for point in points {
            let depth = camera.depth(point);
            if !depth.is_finite() {
                pass.fail("stereo scene has non-finite world geometry");
                return;
            }
            near = near.min(depth);
            far = far.max(depth);
        }
        if far < NEAR {
            return;
        }
        if let Ok(mut state) = pass.state.lock() {
            state.near = state.near.min(near.max(NEAR) / zero);
            state.far = state.far.max(far / zero);
        }
    }

    pub(crate) fn stereo_plane(&self, ctx: &EvalCtx, camera: &ViewCamera, world: Mat4, bounds: [f64; 4]) {
        self.stereo_points(
            ctx,
            camera,
            [(bounds[0], bounds[1]), (bounds[2], bounds[1]), (bounds[2], bounds[3]), (bounds[0], bounds[3])].map(|(x, y)| world.apply(Vec3::from([x, y, 0.0]))),
        );
    }

    pub(crate) fn stereo_owner(&self, ctx: &EvalCtx) -> Option<&Pass> {
        self.stereo.filter(|pass| self.collapse3d.map_or(ctx.comp_id, |collapsed| collapsed.parent.comp_id) == pass.comp)
    }

    pub(crate) fn stereo_infinity(&self, ctx: &EvalCtx) {
        if let Some(pass) = self.stereo_owner(ctx)
            && let Ok(mut state) = pass.state.lock()
        {
            state.infinity = true;
        }
    }

    /// CPU rendering with private effects/caches per eye, one borrowed project and root clock.
    /// Ordinary rendering and acceleration are unchanged. This method never shares a mutable
    /// accelerator or processed-layer cache between eyes.
    pub fn stereo_frame(&self, comp: ItemId, time: Tick, request: StereoRequest) -> Result<StereoFrame, String> {
        if ![request.budget.sunk, request.budget.popped].into_iter().all(|value| value.is_finite() && value >= 0.0) {
            return Err("stereo depth feedback must be finite and nonnegative".into());
        }
        let document = self.project.comp(comp).ok_or_else(|| "stereo composition does not exist".to_string())?;
        let scale = f64::from(request.width) / f64::from(document.width);
        let height = (f64::from(document.height) * scale).round();
        if request.width == 0 || !height.is_finite() || height < 1.0 || f64::from(request.width) * height > 16_000_000.0 {
            return Err("stereo per-eye resolution must fit the renderer's 16 million pixel work limit".into());
        }
        if self.opts.backend != Backend::Cpu {
            return Err("stereo_frame requires the CPU compositor and private eye state".into());
        }
        let opts = RenderOpts { scale, roi: None, ..self.opts };
        let resources = FrozenFootage::new(self.footage);
        resources.check()?;
        let probe = Pass::new(comp, None, request, [1.0, 1.0], false);
        let renderer = Renderer { stereo: Some(&probe), whole_raster: true, expr: self.expr, ..Renderer::new(self.project, &resources, opts) };
        let _ = renderer.comp_frame_cpu(comp, time);
        resources.check()?;
        let observed = probe.finish()?;
        let depths = if observed.near.is_finite() { [observed.near, observed.far] } else { [1.0, 1.0] };
        let infinity = observed.infinity;
        drop(observed);
        let render_eye = |eye| -> Result<(Image, Vec<StereoSample>), String> {
            let pass = Pass::new(comp, Some(eye), request, depths, infinity);
            let renderer = Renderer { stereo: Some(&pass), whole_raster: true, expr: self.expr, ..Renderer::new(self.project, &resources, opts) };
            let image = renderer.comp_frame_cpu(comp, time);
            let state = pass.finish()?;
            resources.check()?;
            let mut samples = state.samples.clone();
            samples.sort_by_key(|sample| sample.time);
            Ok((image, samples))
        };
        let (left, left_samples) = render_eye(0)?;
        let (right, right_samples) = render_eye(1)?;
        if left_samples != right_samples {
            return Err("stereo eyes evaluated different root scene samples".into());
        }
        Ok(StereoFrame { eyes: [left, right], samples: [left_samples, right_samples], depths, resource_queries: resources.counts() })
    }
}
