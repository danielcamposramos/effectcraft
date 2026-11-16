use super::*;
use crate::stereo::{StereoFrame, StereoRequest};
use craft_stereo_camera::Budget;
use effectcraft_keyframe::Keyframe;

fn fixture(levels: usize, advanced: bool, moving: bool, flat: bool) -> (Project, ItemId) {
    let (mut p, id, mut root) = setup();
    root.enable_motion_blur = moving;
    root.shutter_angle = 90.0;
    root.shutter_phase = -45.0;
    root.motion_blur_samples = 7;
    if advanced {
        root.renderer = effectcraft_project::Renderer::Advanced3D;
    }
    let mut cam = camera(&mut p, &root);
    if moving {
        for (path, z) in [("transform/position", -zoom()), ("transform/poi", 0.0)] {
            cam.props.prop_mut(path).unwrap().keys =
                vec![Keyframe::new(Tick::ZERO, Value::Vec3([0.0, 50.0, z])), Keyframe::new(Tick::from_seconds_f64(1.0), Value::Vec3([200.0, 50.0, z]))];
        }
    }
    let mut contents = vec![
        solid3(&mut p, &root, [1.0, 0.0, 0.0], 16, 18, [60.0, 32.0, -zoom() / 5.0]),
        solid3(&mut p, &root, [0.0, 1.0, 0.0], 30, 18, [140.0, 32.0, zoom() / 5.0]),
    ];
    for layer in &mut contents {
        layer.switches.motion_blur = moving;
        if flat {
            layer.switches.three_d = false;
        }
    }
    if advanced {
        let mut cube = build::layer(&mut p, &root, "Cube", LayerSource::Primitive { kind: effectcraft_project::PrimitiveKind::Cube }, (W, H), None);
        cube.switches.motion_blur = moving;
        set(&mut cube, "transform/position", Value::Vec3([100.0, 73.0, 0.0]));
        for dim in ["width", "height", "depth"] {
            set(&mut cube, &format!("geometryOptions/{dim}"), Value::Scalar(24.0));
        }
        set(&mut cube, "materialOptions/baseColor", Value::Color([0.0, 0.0, 1.0, 1.0]));
        contents.push(cube);
    }
    for _ in 0..levels {
        let mut child = root.clone();
        child.frame_rate = FrameRate::FPS_24;
        child.shutter_angle = 360.0;
        child.shutter_phase = 90.0;
        child.motion_blur_samples = 13;
        child.layers = contents.into();
        let cid = p.add_item("Child", Label::Sandstone, None, ItemKind::Comp(child.into()));
        let mut layer = build::layer(&mut p, &root, "Collapsed", LayerSource::Comp { item: cid }, (W, H), None);
        layer.switches.collapse = true;
        layer.switches.three_d = true;
        layer.switches.motion_blur = moving;
        contents = vec![layer];
    }
    let mut layers = vec![cam];
    layers.extend(contents);
    root.layers = layers.into();
    *p.comp_mut(id).unwrap() = root;
    (p, id)
}

fn pair(p: &Project, id: ItemId, width: u32, time: f64) -> StereoFrame {
    Renderer::new(p, &NoFootage, RenderOpts::default())
        .stereo_frame(id, Tick::from_seconds_f64(time), StereoRequest { width, budget: Budget { popped: 12.0, sunk: 8.0 } })
        .unwrap()
}

fn center(image: &effectcraft_raster::Image, color: usize) -> (f64, f64) {
    let mut sum = [0.0; 3];
    for (i, pixel) in image.data.iter().enumerate() {
        let other = (color + 1) % 3;
        if pixel[color] > 0.1 && pixel[color] > pixel[other] * 2.0 {
            let weight = f64::from(pixel[color]);
            sum[0] += (i % image.width as usize) as f64 * weight;
            sum[1] += (i / image.width as usize) as f64 * weight;
            sum[2] += weight;
        }
    }
    assert!(sum[2] > 20.0, "real marker must be visible");
    (sum[0] / sum[2], sum[1] / sum[2])
}

#[test]
fn actual_geometry_has_predicted_disparity_at_two_native_grids() {
    for advanced in [false, true] {
        let (p, id) = fixture(0, advanced, false, false);
        for width in [200, 640] {
            let pair = pair(&p, id, width, 0.5);
            for (color, prediction) in [(0, 12.0), (1, -8.0)] {
                let a = center(&pair.eyes[0], color);
                let b = center(&pair.eyes[1], color);
                assert!((a.0 - b.0 - prediction).abs() < 0.7, "{advanced} {width}: {:?} {:?}", a, b);
                assert!((a.1 - b.1).abs() < 1e-5);
                assert!((b.0 - a.0 - prediction).abs() > 10.0, "wrong eye control must fail");
            }
        }
    }
}

#[test]
fn root_shutter_pair_survives_collapsed_composition_instances() {
    for advanced in [false, true] {
        let (p, id) = fixture(0, advanced, true, false);
        let direct = pair(&p, id, 200, 0.5);
        for levels in [1, 2] {
            let (p, id) = fixture(levels, advanced, true, false);
            let nested = pair(&p, id, 200, 0.5);
            for (a, b) in direct.eyes.iter().zip(&nested.eyes) {
                let max = a.data.iter().zip(&b.data).flat_map(|(a, b)| a.iter().zip(b).map(|(a, b)| (a - b).abs())).fold(0.0, f32::max);
                assert!(max < 1e-5, "{advanced} {levels}: {max}");
            }
        }
        assert_eq!(direct.samples[0], direct.samples[1]);
        assert!(direct.samples[0].iter().any(|sample| sample.time != Tick::from_seconds_f64(0.5)), "actual root shutter times must be sampled");
        let later = pair(&p, id, 200, 0.6);
        for (a, b) in direct.eyes.iter().zip(&later.eyes) {
            assert_ne!(a.data, b.data, "animated camera must not freeze");
        }
    }
}

#[test]
fn flat_art_equals_the_ordinary_renderer_in_every_pixel() {
    let (p, id) = fixture(0, false, false, true);
    let expected = Renderer::new(&p, &NoFootage, RenderOpts::default()).comp_frame(id, Tick::ZERO);
    let pair = pair(&p, id, 200, 0.0);
    for eye in pair.eyes {
        assert_eq!(eye.data, expected.data);
    }
}

#[test]
fn invalid_request_is_a_visible_error() {
    let (p, id) = fixture(0, false, false, false);
    let renderer = Renderer::new(&p, &NoFootage, RenderOpts::default());
    for width in [0, u32::MAX] {
        assert!(renderer.stereo_frame(id, Tick::ZERO, StereoRequest { width, budget: Budget { popped: 12.0, sunk: 8.0 } }).is_err());
    }
}

#[test]
fn a_zero_depth_budget_restores_the_central_scene_exactly() {
    for advanced in [false, true] {
        let (p, id) = fixture(2, advanced, true, false);
        let renderer = Renderer::new(&p, &NoFootage, RenderOpts::default());
        let time = Tick::from_seconds_f64(0.5);
        let expected = renderer.comp_frame(id, time);
        let pair = renderer.stereo_frame(id, time, StereoRequest { width: 200, budget: Budget { popped: 0.0, sunk: 0.0 } }).unwrap();
        for eye in pair.eyes {
            assert_eq!(eye.data, expected.data);
        }
    }
}

#[test]
fn flat_scenes_also_reject_invalid_live_depth_feedback() {
    let (p, id) = fixture(0, false, false, true);
    for value in [f32::NAN, -1.0, f32::INFINITY] {
        assert!(
            Renderer::new(&p, &NoFootage, RenderOpts::default())
                .stereo_frame(id, Tick::ZERO, StereoRequest { width: 200, budget: Budget { popped: value, sunk: 8.0 } })
                .is_err()
        );
    }
}

#[test]
fn camera_effects_without_a_geometry_bound_refuse_a_false_stereo_success() {
    let (mut project, id) = fixture(0, false, false, true);
    let mut layer = project.comp(id).unwrap().layers[1].clone();
    let spec = effectcraft_effects::find("ec.sim.carddance").unwrap();
    let mut ids = build::Ids(&mut project.next_id);
    let mut effect = effectcraft_effects::instantiate(spec, &mut ids, spec.name, [16.0, 18.0]);
    effect.prop_mut("cameraSystem").unwrap().value = Value::Enum(2);
    layer.props.sub_mut("effects").unwrap().children.push(effect.into());
    project.comp_mut(id).unwrap().layers[1] = layer;
    let renderer = Renderer::new(&project, &NoFootage, RenderOpts::default());
    let ordinary = renderer.comp_frame(id, Tick::ZERO);
    assert!(ordinary.data.iter().any(|pixel| pixel[0] > 0.5));
    let result = renderer.stereo_frame(id, Tick::ZERO, StereoRequest { width: 200, budget: Budget { popped: 12.0, sunk: 8.0 } });
    assert!(matches!(result,Err(error) if error.contains("effect") && error.contains("depth")), "a camera-aware flattened effect cannot invent an eye");
}

#[test]
fn a_one_node_camera_ignores_its_inactive_point_of_interest() {
    let (mut project, id) = fixture(0, false, false, false);
    let expected = pair(&project, id, 200, 0.5);
    let cam = &mut project.comp_mut(id).unwrap().layers[0];
    cam.auto_orient = effectcraft_project::AutoOrient::Off;
    set(cam, "transform/poi", Value::Vec3([23.0, 17.0, 700.0]));
    let actual = pair(&project, id, 200, 0.5);
    for (a, b) in expected.eyes.iter().zip(&actual.eyes) {
        assert!(a.data == b.data, "an inactive property must not move the stereo zero plane");
    }
}

#[test]
fn an_uncollapsed_composition_keeps_its_authored_flat_camera_boundary() {
    let (mut project, id) = fixture(0, false, false, false);
    let root = project.comp(id).unwrap().clone();
    let mut inner = Comp::new(W, H, FrameRate::FPS_30, Tick::from_seconds_f64(2.0));
    let camera = camera(&mut project, &inner);
    let blue = solid3(&mut project, &inner, [0.0, 0.0, 1.0], 4, 4, [100.0, 50.0, -zoom() * 0.75]);
    inner.layers = vec![camera, blue].into();
    let inner_id = project.add_item("Inner world", Label::Sandstone, None, ItemKind::Comp(inner.into()));
    let mut wrapper = Comp::new(W, H, FrameRate::FPS_30, Tick::from_seconds_f64(2.0));
    let mut instance = build::layer(&mut project, &wrapper, "Internal collapse", LayerSource::Comp { item: inner_id }, (W, H), None);
    instance.switches.collapse = true;
    instance.switches.three_d = true;
    wrapper.layers = vec![instance].into();
    let wrapper_id = project.add_item("Flat boundary", Label::Sandstone, None, ItemKind::Comp(wrapper.into()));
    let flat = build::layer(&mut project, &root, "Authored flat precomp", LayerSource::Comp { item: wrapper_id }, (W, H), None);
    assert!(!flat.switches.collapse && !flat.is_3d());
    project.comp_mut(id).unwrap().layers.push(flat);
    let result = pair(&project, id, 200, 0.5);
    assert!((result.depths[0] - 0.8).abs() < 1e-8, "internal geometry must not change the root envelope: {:?}", result.depths);
    assert!((result.depths[1] - 1.2).abs() < 1e-8);
    let a = center(&result.eyes[0], 2);
    let b = center(&result.eyes[1], 2);
    assert!((a.0 - b.0).abs() < 1e-8 && (a.1 - b.1).abs() < 1e-8, "an authored flat precomp stays flat: {a:?} {b:?}");
}

#[test]
fn decoded_scene_inputs_are_frozen_before_either_eye_is_rendered() {
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };
    struct ChangingSource(AtomicUsize);
    impl crate::FootageSource for ChangingSource {
        fn frame(&self, _item: ItemId, _footage: &effectcraft_project::Footage, _time: Tick) -> Option<Arc<crate::Image>> {
            let n = self.0.fetch_add(1, Ordering::SeqCst);
            let mut image = crate::Image::new(W, H);
            image.data.fill(if n.is_multiple_of(2) { [1.0, 0.0, 0.0, 1.0] } else { [0.0, 0.0, 1.0, 1.0] });
            Some(Arc::new(image))
        }
    }
    let (mut project, id, mut comp) = setup();
    let footage = effectcraft_project::Footage { path: "synthetic-pending-image.png".into(), width: W, height: H, has_video: true, ..Default::default() };
    let source_id = project.add_item("Changing source", Label::Red, None, ItemKind::Footage(footage));
    let layer = build::layer(&mut project, &comp, "Shared scene image", LayerSource::Footage { item: source_id }, (W, H), None);
    comp.layers.push(layer);
    *project.comp_mut(id).unwrap() = comp;
    let source = ChangingSource(AtomicUsize::new(0));
    let pair = Renderer::new(&project, &source, RenderOpts::default())
        .stereo_frame(id, Tick::ZERO, StereoRequest { width: 200, budget: Budget { popped: 12.0, sunk: 8.0 } })
        .unwrap();
    assert!(pair.eyes[0].data == pair.eyes[1].data, "the same composition time cannot resolve two different source worlds");
    assert_eq!(source.0.load(Ordering::SeqCst), 1, "preparation and both eye renders must share one decoded scene input");
    assert!(pair.eyes[0].get(100, 50)[0] > 0.9);
}

#[test]
fn an_unavailable_required_scene_image_is_an_error_not_a_blank_pair() {
    let (mut project, id, mut comp) = setup();
    let footage = effectcraft_project::Footage { path: "synthetic-unavailable.png".into(), width: W, height: H, has_video: true, ..Default::default() };
    let input = project.add_item("Unavailable image", Label::Red, None, ItemKind::Footage(footage));
    comp.layers.push(build::layer(&mut project, &comp, "Required scene input", LayerSource::Footage { item: input }, (W, H), None));
    *project.comp_mut(id).unwrap() = comp;
    let result = Renderer::new(&project, &NoFootage, RenderOpts::default()).stereo_frame(
        id,
        Tick::ZERO,
        StereoRequest { width: 200, budget: Budget { popped: 12.0, sunk: 8.0 } },
    );
    assert!(
        matches!(result,Err(error) if error.contains("input")||error.contains("unavailable")),
        "an unresolved source cannot be published as a complete world"
    );
}
