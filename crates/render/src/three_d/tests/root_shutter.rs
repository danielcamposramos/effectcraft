//! The selected root camera follows the root shutter through identity instances.
//! Install as a child of three_d::tests so the existing fixture helpers stay shared.
use super::*;
use effectcraft_keyframe::Keyframe;
use effectcraft_project::Renderer as SceneRenderer;

fn root_camera_frame(levels: usize, held: bool, mode: SceneRenderer, split_shutters: bool, container_3d: bool) -> effectcraft_raster::Image {
    root_camera_frame_at(levels, held, mode, split_shutters, container_3d, Tick::from_seconds_f64(0.5))
}

#[derive(Clone, Copy, Debug)]
enum InstanceClock {
    Identity,
    Faster,
    Slower,
    Remapped,
}

fn root_camera_frame_at(levels: usize, held: bool, mode: SceneRenderer, split_shutters: bool, container_3d: bool, time: Tick) -> effectcraft_raster::Image {
    root_camera_frame_with_clock(levels, held, mode, split_shutters, container_3d, time, InstanceClock::Identity)
}

fn root_camera_frame_with_clock(
    levels: usize,
    held: bool,
    mode: SceneRenderer,
    split_shutters: bool,
    container_3d: bool,
    time: Tick,
    clock: InstanceClock,
) -> effectcraft_raster::Image {
    root_camera_scene(levels, held, mode, split_shutters, container_3d, time, RootFixture { clock, ..Default::default() })
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum BlurScope {
    Enabled,
    RenderOff,
    RootOff,
}

#[derive(Clone, Copy)]
struct RootFixture {
    clock: InstanceClock,
    mixed: bool,
    animated_light: bool,
    held_light: bool,
    blur: BlurScope,
    collapsed: bool,
}

impl Default for RootFixture {
    fn default() -> Self {
        Self { clock: InstanceClock::Identity, mixed: false, animated_light: false, held_light: false, blur: BlurScope::Enabled, collapsed: true }
    }
}

fn root_camera_scene(
    levels: usize,
    held: bool,
    mode: SceneRenderer,
    split_shutters: bool,
    container_3d: bool,
    time: Tick,
    fixture: RootFixture,
) -> effectcraft_raster::Image {
    let (mut project, root_id, mut root) = setup();
    root.renderer = mode;
    root.enable_motion_blur = fixture.blur != BlurScope::RootOff;
    root.shutter_angle = 90.0;
    root.shutter_phase = -45.0;
    root.motion_blur_samples = 7;
    let mut camera = camera(&mut project, &root);
    for (path, z) in [("transform/position", -zoom()), ("transform/poi", 0.0)] {
        camera.props.prop_mut(path).unwrap().keys =
            vec![Keyframe::new(Tick::ZERO, Value::Vec3([0.0, 50.0, z])), Keyframe::new(Tick::from_seconds_f64(1.0), Value::Vec3([200.0, 50.0, z]))];
    }
    let mut layer = solid3(&mut project, &root, [1.0, 0.0, 0.0], 40, 20, [100.0, 50.0, 0.0]);
    layer.switches.motion_blur = true;
    let mut content = vec![layer];
    if fixture.mixed {
        set(&mut content[0], "transform/position", Value::Vec3([70.0, 50.0, 20.0]));
        let mut cube = build::layer(&mut project, &root, "Cube", LayerSource::Primitive { kind: effectcraft_project::PrimitiveKind::Cube }, (W, H), None);
        cube.switches.motion_blur = true;
        set(&mut cube, "transform/position", Value::Vec3([130.0, 50.0, -30.0]));
        for dimension in ["width", "height", "depth"] {
            set(&mut cube, &format!("geometryOptions/{dimension}"), Value::Scalar(24.0));
        }
        set(&mut cube, "materialOptions/baseColor", Value::Color([0.0, 1.0, 0.0, 1.0]));
        content.push(cube);
    }
    for level in 0..levels {
        let mut child = if split_shutters { Comp::new(W, H, FrameRate::FPS_24, Tick::from_seconds_f64(2.0)) } else { root.clone() };
        child.enable_motion_blur = true;
        child.renderer = mode;
        if split_shutters {
            child.shutter_angle = 360.0;
            child.shutter_phase = 90.0;
            child.motion_blur_samples = 13;
        }
        child.layers = content.into();
        let id = project.add_item(&format!("Identity {level}"), Label::Sandstone, None, ItemKind::Comp(child.into()));
        let mut layer = build::layer(&mut project, &root, "Identity", LayerSource::Comp { item: id }, (W, H), None);
        layer.switches.collapse = fixture.collapsed;
        layer.switches.three_d = container_3d;
        layer.switches.motion_blur = true;
        match fixture.clock {
            InstanceClock::Identity => {}
            InstanceClock::Faster => layer.stretch = 50.0,
            InstanceClock::Slower => layer.stretch = 200.0,
            InstanceClock::Remapped => {
                let mut remap = build::Ids(&mut project.next_id).prop("timeRemap", "Time Remap", Value::Scalar(0.0));
                remap.keys = [(0.0, 0.0), (0.5, 0.25), (1.0, 1.0)]
                    .into_iter()
                    .map(|(time, value)| Keyframe::new(Tick::from_seconds_f64(time), Value::Scalar(value)))
                    .collect();
                layer.props.children.push(effectcraft_project::Node::Prop(remap));
            }
        }
        content = vec![layer];
    }
    let mut layers = vec![camera];
    if fixture.animated_light {
        let mut light = build::layer(&mut project, &root, "Ambient", LayerSource::Light { kind: LightKind::Ambient }, (W, H), None);
        let intensity = light.props.prop_mut("lightOptions/intensity").unwrap();
        intensity.value = Value::Scalar(100.0);
        if !fixture.held_light {
            intensity.keys = [(0.49, 0.0), (0.5, 100.0), (0.51, 0.0)]
                .into_iter()
                .map(|(time, value)| Keyframe::new(Tick::from_seconds_f64(time), Value::Scalar(value)))
                .collect();
        }
        layers.push(light);
    }
    layers.extend(content);
    root.layers = layers.into();
    *project.comp_mut(root_id).unwrap() = root;
    let mut opts = RenderOpts { motion_blur: fixture.blur != BlurScope::RenderOff, ..Default::default() };
    if held {
        let renderer = Renderer::new(&project, &NoFootage, opts);
        let context = renderer.eval_ctx(root_id, time).unwrap();
        opts.view = Some(super::super::camera::frame_camera(&context, time));
    }
    let image = Renderer::new(&project, &NoFootage, opts).comp_frame(root_id, time);
    if fixture.mixed {
        assert!(image.data.iter().filter(|p| p[3] > 0.5 && p[1] > 0.1 && p[1] > p[0] * 2.0).count() > 20, "the real mesh must be visible");
        assert!(image.data.iter().filter(|p| p[3] > 0.5 && p[0] > 0.1 && p[0] > p[1] * 2.0).count() > 20, "the card must be visible");
    }
    image
}

fn image_difference(a: &effectcraft_raster::Image, b: &effectcraft_raster::Image) -> f32 {
    assert_eq!((a.width, a.height), (b.width, b.height));
    a.data.iter().zip(&b.data).flat_map(|(a, b)| a.iter().zip(b).map(|(a, b)| (a - b).abs())).fold(0.0, f32::max)
}

fn root_camera_moves(mode: SceneRenderer) {
    let moving = root_camera_frame(0, false, mode, false, true);
    let held = root_camera_frame(0, true, mode, false, true);
    assert!(image_difference(&moving, &held) > 0.05, "the control must observe camera motion");
}

fn root_camera_instance(mode: SceneRenderer, levels: usize, split: bool, container_3d: bool) {
    let moving = root_camera_frame(0, false, mode, false, true);
    let nested = root_camera_frame(levels, false, mode, split, container_3d);
    let difference = image_difference(&moving, &nested);
    assert!(difference < 1e-5, "{mode:?}, levels={levels}, distinct child shutter={split}, container3D={container_3d}: {difference}");
}

#[test]
fn classic_root_shutter_control_observes_camera_motion() {
    root_camera_moves(SceneRenderer::Classic3D);
}

#[test]
fn classic_root_shutter_1_instances_same_child_shutter_2d_container() {
    root_camera_instance(SceneRenderer::Classic3D, 1, false, false);
}

#[test]
fn classic_root_shutter_1_instances_same_child_shutter_3d_container() {
    root_camera_instance(SceneRenderer::Classic3D, 1, false, true);
}

#[test]
fn classic_root_shutter_1_instances_distinct_child_shutter_2d_container() {
    root_camera_instance(SceneRenderer::Classic3D, 1, true, false);
}

#[test]
fn classic_root_shutter_1_instances_distinct_child_shutter_3d_container() {
    root_camera_instance(SceneRenderer::Classic3D, 1, true, true);
}

#[test]
fn classic_root_shutter_2_instances_same_child_shutter_2d_container() {
    root_camera_instance(SceneRenderer::Classic3D, 2, false, false);
}

#[test]
fn classic_root_shutter_2_instances_same_child_shutter_3d_container() {
    root_camera_instance(SceneRenderer::Classic3D, 2, false, true);
}

#[test]
fn classic_root_shutter_2_instances_distinct_child_shutter_2d_container() {
    root_camera_instance(SceneRenderer::Classic3D, 2, true, false);
}

#[test]
fn classic_root_shutter_2_instances_distinct_child_shutter_3d_container() {
    root_camera_instance(SceneRenderer::Classic3D, 2, true, true);
}

#[test]
fn advanced_root_shutter_control_observes_camera_motion() {
    root_camera_moves(SceneRenderer::Advanced3D);
}

#[test]
fn advanced_root_shutter_1_instances_same_child_shutter_2d_container() {
    root_camera_instance(SceneRenderer::Advanced3D, 1, false, false);
}

#[test]
fn advanced_root_shutter_1_instances_same_child_shutter_3d_container() {
    root_camera_instance(SceneRenderer::Advanced3D, 1, false, true);
}

#[test]
fn advanced_root_shutter_1_instances_distinct_child_shutter_2d_container() {
    root_camera_instance(SceneRenderer::Advanced3D, 1, true, false);
}

#[test]
fn advanced_root_shutter_1_instances_distinct_child_shutter_3d_container() {
    root_camera_instance(SceneRenderer::Advanced3D, 1, true, true);
}

#[test]
fn advanced_root_shutter_2_instances_same_child_shutter_2d_container() {
    root_camera_instance(SceneRenderer::Advanced3D, 2, false, false);
}

#[test]
fn advanced_root_shutter_2_instances_same_child_shutter_3d_container() {
    root_camera_instance(SceneRenderer::Advanced3D, 2, false, true);
}

#[test]
fn advanced_root_shutter_2_instances_distinct_child_shutter_2d_container() {
    root_camera_instance(SceneRenderer::Advanced3D, 2, true, false);
}

#[test]
fn advanced_root_shutter_2_instances_distinct_child_shutter_3d_container() {
    root_camera_instance(SceneRenderer::Advanced3D, 2, true, true);
}

fn explicit_root_view_is_held(mode: SceneRenderer) {
    let direct = root_camera_frame(0, true, mode, false, true);
    for levels in [1, 2] {
        for container_3d in [false, true] {
            let nested = root_camera_frame(levels, true, mode, true, container_3d);
            assert!(image_difference(&direct, &nested) < 1e-5, "an explicit view override stays held through {levels} instances, container3D={container_3d}");
        }
    }
}

#[test]
fn classic_explicit_root_view_remains_held() {
    explicit_root_view_is_held(SceneRenderer::Classic3D);
}

#[test]
fn advanced_explicit_root_view_remains_held() {
    explicit_root_view_is_held(SceneRenderer::Advanced3D);
}

fn first_frame_keeps_the_selected_root_camera(mode: SceneRenderer) {
    let direct = root_camera_frame_at(0, false, mode, false, true, Tick::ZERO);
    for levels in [1, 2] {
        for container_3d in [false, true] {
            let nested = root_camera_frame_at(levels, false, mode, false, container_3d, Tick::ZERO);
            assert!(
                image_difference(&direct, &nested) < 1e-5,
                "the selected root camera remains the shutter camera on frame zero: levels={levels}, container3D={container_3d}"
            );
        }
    }
}

#[test]
fn classic_first_frame_keeps_the_selected_root_camera() {
    first_frame_keeps_the_selected_root_camera(SceneRenderer::Classic3D);
}

#[test]
fn advanced_first_frame_keeps_the_selected_root_camera() {
    first_frame_keeps_the_selected_root_camera(SceneRenderer::Advanced3D);
}

fn mapped_child_clock_does_not_replace_root_camera_time(mode: SceneRenderer, levels: usize, container_3d: bool, clock: InstanceClock) {
    // Even two 50% instances stay within the two-second child span over this shutter.
    let time = Tick::from_seconds_f64(0.25);
    let direct = root_camera_frame_at(0, false, mode, false, true, time);
    let nested = root_camera_frame_with_clock(levels, false, mode, true, container_3d, time, clock);
    assert!(image_difference(&direct, &nested) < 1e-5, "root time is independent of {clock:?}: levels={levels}, container3D={container_3d}");
}

#[test]
fn classic_root_camera_time_1_instances_faster_2d() {
    mapped_child_clock_does_not_replace_root_camera_time(SceneRenderer::Classic3D, 1, false, InstanceClock::Faster);
}

#[test]
fn classic_root_camera_time_1_instances_slower_2d() {
    mapped_child_clock_does_not_replace_root_camera_time(SceneRenderer::Classic3D, 1, false, InstanceClock::Slower);
}

#[test]
fn classic_root_camera_time_1_instances_nonlinear_2d() {
    mapped_child_clock_does_not_replace_root_camera_time(SceneRenderer::Classic3D, 1, false, InstanceClock::Remapped);
}

#[test]
fn classic_root_camera_time_1_instances_faster_3d() {
    mapped_child_clock_does_not_replace_root_camera_time(SceneRenderer::Classic3D, 1, true, InstanceClock::Faster);
}

#[test]
fn classic_root_camera_time_1_instances_slower_3d() {
    mapped_child_clock_does_not_replace_root_camera_time(SceneRenderer::Classic3D, 1, true, InstanceClock::Slower);
}

#[test]
fn classic_root_camera_time_1_instances_nonlinear_3d() {
    mapped_child_clock_does_not_replace_root_camera_time(SceneRenderer::Classic3D, 1, true, InstanceClock::Remapped);
}

#[test]
fn classic_root_camera_time_2_instances_faster_2d() {
    mapped_child_clock_does_not_replace_root_camera_time(SceneRenderer::Classic3D, 2, false, InstanceClock::Faster);
}

#[test]
fn classic_root_camera_time_2_instances_slower_2d() {
    mapped_child_clock_does_not_replace_root_camera_time(SceneRenderer::Classic3D, 2, false, InstanceClock::Slower);
}

#[test]
fn classic_root_camera_time_2_instances_nonlinear_2d() {
    mapped_child_clock_does_not_replace_root_camera_time(SceneRenderer::Classic3D, 2, false, InstanceClock::Remapped);
}

#[test]
fn classic_root_camera_time_2_instances_faster_3d() {
    mapped_child_clock_does_not_replace_root_camera_time(SceneRenderer::Classic3D, 2, true, InstanceClock::Faster);
}

#[test]
fn classic_root_camera_time_2_instances_slower_3d() {
    mapped_child_clock_does_not_replace_root_camera_time(SceneRenderer::Classic3D, 2, true, InstanceClock::Slower);
}

#[test]
fn classic_root_camera_time_2_instances_nonlinear_3d() {
    mapped_child_clock_does_not_replace_root_camera_time(SceneRenderer::Classic3D, 2, true, InstanceClock::Remapped);
}

#[test]
fn advanced_root_camera_time_1_instances_faster_2d() {
    mapped_child_clock_does_not_replace_root_camera_time(SceneRenderer::Advanced3D, 1, false, InstanceClock::Faster);
}

#[test]
fn advanced_root_camera_time_1_instances_slower_2d() {
    mapped_child_clock_does_not_replace_root_camera_time(SceneRenderer::Advanced3D, 1, false, InstanceClock::Slower);
}

#[test]
fn advanced_root_camera_time_1_instances_nonlinear_2d() {
    mapped_child_clock_does_not_replace_root_camera_time(SceneRenderer::Advanced3D, 1, false, InstanceClock::Remapped);
}

#[test]
fn advanced_root_camera_time_1_instances_faster_3d() {
    mapped_child_clock_does_not_replace_root_camera_time(SceneRenderer::Advanced3D, 1, true, InstanceClock::Faster);
}

#[test]
fn advanced_root_camera_time_1_instances_slower_3d() {
    mapped_child_clock_does_not_replace_root_camera_time(SceneRenderer::Advanced3D, 1, true, InstanceClock::Slower);
}

#[test]
fn advanced_root_camera_time_1_instances_nonlinear_3d() {
    mapped_child_clock_does_not_replace_root_camera_time(SceneRenderer::Advanced3D, 1, true, InstanceClock::Remapped);
}

#[test]
fn advanced_root_camera_time_2_instances_faster_2d() {
    mapped_child_clock_does_not_replace_root_camera_time(SceneRenderer::Advanced3D, 2, false, InstanceClock::Faster);
}

#[test]
fn advanced_root_camera_time_2_instances_slower_2d() {
    mapped_child_clock_does_not_replace_root_camera_time(SceneRenderer::Advanced3D, 2, false, InstanceClock::Slower);
}

#[test]
fn advanced_root_camera_time_2_instances_nonlinear_2d() {
    mapped_child_clock_does_not_replace_root_camera_time(SceneRenderer::Advanced3D, 2, false, InstanceClock::Remapped);
}

#[test]
fn advanced_root_camera_time_2_instances_faster_3d() {
    mapped_child_clock_does_not_replace_root_camera_time(SceneRenderer::Advanced3D, 2, true, InstanceClock::Faster);
}

#[test]
fn advanced_root_camera_time_2_instances_slower_3d() {
    mapped_child_clock_does_not_replace_root_camera_time(SceneRenderer::Advanced3D, 2, true, InstanceClock::Slower);
}

#[test]
fn advanced_root_camera_time_2_instances_nonlinear_3d() {
    mapped_child_clock_does_not_replace_root_camera_time(SceneRenderer::Advanced3D, 2, true, InstanceClock::Remapped);
}

fn essential_position_frame(levels: usize, mode: SceneRenderer, container_3d: bool) -> effectcraft_raster::Image {
    use effectcraft_project::essential::{self, EgControl, EgKind, EssentialGraphics};
    let (mut project, root_id, mut root) = setup();
    root.renderer = mode;
    root.enable_motion_blur = true;
    root.shutter_angle = 90.0;
    root.shutter_phase = -45.0;
    root.motion_blur_samples = 7;
    let mut camera = camera(&mut project, &root);
    for (path, z) in [("transform/position", -zoom()), ("transform/poi", 0.0)] {
        camera.props.prop_mut(path).unwrap().keys =
            vec![Keyframe::new(Tick::ZERO, Value::Vec3([0.0, 50.0, z])), Keyframe::new(Tick::from_seconds_f64(1.0), Value::Vec3([200.0, 50.0, z]))];
    }
    let keys = || vec![Keyframe::new(Tick::ZERO, Value::Vec3([80.0, 50.0, 0.0])), Keyframe::new(Tick::from_seconds_f64(1.0), Value::Vec3([120.0, 50.0, 0.0]))];
    let mut layer = solid3(&mut project, &root, [1.0, 0.0, 0.0], 40, 20, [100.0, 50.0, 0.0]);
    layer.switches.motion_blur = true;
    let mut target = layer.props.prop("transform/position").unwrap().uid;
    if levels == 0 {
        layer.props.prop_mut("transform/position").unwrap().keys = keys();
    }
    for level in 0..levels {
        let mut child = Comp::new(W, H, FrameRate::FPS_24, Tick::from_seconds_f64(2.0));
        child.renderer = mode;
        child.enable_motion_blur = true;
        child.shutter_angle = 360.0;
        child.shutter_phase = 90.0;
        child.motion_blur_samples = 13;
        child.essential = Some(EssentialGraphics {
            controls: vec![EgControl {
                id: 1,
                name: "Position".into(),
                kind: EgKind::Property { layer: layer.id, prop: target, links: vec![] },
                as_type: None,
            }],
            ..Default::default()
        });
        child.layers = vec![layer].into();
        let id = project.add_item(&format!("Essential {level}"), Label::Sandstone, None, ItemKind::Comp(child.into()));
        layer = build::layer(&mut project, &root, "Essential", LayerSource::Comp { item: id }, (W, H), None);
        layer.switches.collapse = true;
        layer.switches.three_d = container_3d;
        layer.switches.motion_blur = true;
        let mut ids = build::Ids(&mut project.next_id);
        let mut control = ids.prop(&essential::match_id(1), "Position", Value::Vec3([100.0, 50.0, 0.0]));
        if level + 1 == levels {
            control.keys = keys();
        }
        target = control.uid;
        let mut group = ids.group(essential::GROUP, "Essential Properties");
        group.kind = effectcraft_project::GroupKind::Essential { overridden: vec![control.uid] };
        group.children.push(control.into());
        layer.props.children.push(group.into());
    }
    root.layers = vec![camera, layer].into();
    *project.comp_mut(root_id).unwrap() = root;
    Renderer::new(&project, &NoFootage, RenderOpts::default()).comp_frame(root_id, Tick::from_seconds_f64(0.5))
}

fn essential_position_follows_root_sample(mode: SceneRenderer, levels: usize, container_3d: bool) {
    let direct = essential_position_frame(0, mode, true);
    let nested = essential_position_frame(levels, mode, container_3d);
    assert!(
        image_difference(&direct, &nested) < 1e-5,
        "each ancestor keeps its sampled Essential Property context: levels={levels}, container3D={container_3d}"
    );
}

#[test]
fn classic_root_sample_essential_position_1_2d() {
    essential_position_follows_root_sample(SceneRenderer::Classic3D, 1, false);
}

#[test]
fn classic_root_sample_essential_position_1_3d() {
    essential_position_follows_root_sample(SceneRenderer::Classic3D, 1, true);
}

#[test]
fn classic_root_sample_essential_position_2_2d() {
    essential_position_follows_root_sample(SceneRenderer::Classic3D, 2, false);
}

#[test]
fn classic_root_sample_essential_position_2_3d() {
    essential_position_follows_root_sample(SceneRenderer::Classic3D, 2, true);
}

#[test]
fn advanced_root_sample_essential_position_1_2d() {
    essential_position_follows_root_sample(SceneRenderer::Advanced3D, 1, false);
}

#[test]
fn advanced_root_sample_essential_position_1_3d() {
    essential_position_follows_root_sample(SceneRenderer::Advanced3D, 1, true);
}

#[test]
fn advanced_root_sample_essential_position_2_2d() {
    essential_position_follows_root_sample(SceneRenderer::Advanced3D, 2, false);
}

#[test]
fn advanced_root_sample_essential_position_2_3d() {
    essential_position_follows_root_sample(SceneRenderer::Advanced3D, 2, true);
}

#[test]
fn classic_root_shutter_animated_light_1_instance_2d() {
    let fixture = RootFixture { animated_light: true, ..Default::default() };
    let time = Tick::from_seconds_f64(0.5);
    let direct = root_camera_scene(0, false, SceneRenderer::Classic3D, false, true, time, fixture);
    let nested = root_camera_scene(1, false, SceneRenderer::Classic3D, true, false, time, fixture);
    assert!(image_difference(&direct, &nested) < 1e-5, "the collapsed animated_light scene follows the same root sample");
}

#[test]
fn classic_root_shutter_animated_light_1_instance_3d() {
    let fixture = RootFixture { animated_light: true, ..Default::default() };
    let time = Tick::from_seconds_f64(0.5);
    let direct = root_camera_scene(0, false, SceneRenderer::Classic3D, false, true, time, fixture);
    let nested = root_camera_scene(1, false, SceneRenderer::Classic3D, true, true, time, fixture);
    assert!(image_difference(&direct, &nested) < 1e-5, "the collapsed animated_light scene follows the same root sample");
}

#[test]
fn classic_root_shutter_animated_light_2_instance_2d() {
    let fixture = RootFixture { animated_light: true, ..Default::default() };
    let time = Tick::from_seconds_f64(0.5);
    let direct = root_camera_scene(0, false, SceneRenderer::Classic3D, false, true, time, fixture);
    let nested = root_camera_scene(2, false, SceneRenderer::Classic3D, true, false, time, fixture);
    assert!(image_difference(&direct, &nested) < 1e-5, "the collapsed animated_light scene follows the same root sample");
}

#[test]
fn classic_root_shutter_animated_light_2_instance_3d() {
    let fixture = RootFixture { animated_light: true, ..Default::default() };
    let time = Tick::from_seconds_f64(0.5);
    let direct = root_camera_scene(0, false, SceneRenderer::Classic3D, false, true, time, fixture);
    let nested = root_camera_scene(2, false, SceneRenderer::Classic3D, true, true, time, fixture);
    assert!(image_difference(&direct, &nested) < 1e-5, "the collapsed animated_light scene follows the same root sample");
}

#[test]
fn classic_root_light_control_observes_shutter_samples() {
    let time = Tick::from_seconds_f64(0.5);
    let fixture = RootFixture { animated_light: true, ..Default::default() };
    let moving = root_camera_scene(0, true, SceneRenderer::Classic3D, false, true, time, fixture);
    let held = root_camera_scene(0, true, SceneRenderer::Classic3D, false, true, time, RootFixture { held_light: true, ..fixture });
    assert!(image_difference(&moving, &held) > 0.05, "the control must observe the light changing within the shutter");
}

#[test]
fn advanced_root_shutter_animated_light_1_instance_2d() {
    let fixture = RootFixture { animated_light: true, ..Default::default() };
    let time = Tick::from_seconds_f64(0.5);
    let direct = root_camera_scene(0, false, SceneRenderer::Advanced3D, false, true, time, fixture);
    let nested = root_camera_scene(1, false, SceneRenderer::Advanced3D, true, false, time, fixture);
    assert!(image_difference(&direct, &nested) < 1e-5, "the collapsed animated_light scene follows the same root sample");
}

#[test]
fn advanced_root_shutter_mixed_1_instance_2d() {
    let fixture = RootFixture { mixed: true, ..Default::default() };
    let time = Tick::from_seconds_f64(0.5);
    let direct = root_camera_scene(0, false, SceneRenderer::Advanced3D, false, true, time, fixture);
    let nested = root_camera_scene(1, false, SceneRenderer::Advanced3D, true, false, time, fixture);
    assert!(image_difference(&direct, &nested) < 1e-5, "the collapsed mixed scene follows the same root sample");
}

#[test]
fn advanced_root_shutter_animated_light_1_instance_3d() {
    let fixture = RootFixture { animated_light: true, ..Default::default() };
    let time = Tick::from_seconds_f64(0.5);
    let direct = root_camera_scene(0, false, SceneRenderer::Advanced3D, false, true, time, fixture);
    let nested = root_camera_scene(1, false, SceneRenderer::Advanced3D, true, true, time, fixture);
    assert!(image_difference(&direct, &nested) < 1e-5, "the collapsed animated_light scene follows the same root sample");
}

#[test]
fn advanced_root_shutter_mixed_1_instance_3d() {
    let fixture = RootFixture { mixed: true, ..Default::default() };
    let time = Tick::from_seconds_f64(0.5);
    let direct = root_camera_scene(0, false, SceneRenderer::Advanced3D, false, true, time, fixture);
    let nested = root_camera_scene(1, false, SceneRenderer::Advanced3D, true, true, time, fixture);
    assert!(image_difference(&direct, &nested) < 1e-5, "the collapsed mixed scene follows the same root sample");
}

#[test]
fn advanced_root_shutter_animated_light_2_instance_2d() {
    let fixture = RootFixture { animated_light: true, ..Default::default() };
    let time = Tick::from_seconds_f64(0.5);
    let direct = root_camera_scene(0, false, SceneRenderer::Advanced3D, false, true, time, fixture);
    let nested = root_camera_scene(2, false, SceneRenderer::Advanced3D, true, false, time, fixture);
    assert!(image_difference(&direct, &nested) < 1e-5, "the collapsed animated_light scene follows the same root sample");
}

#[test]
fn advanced_root_shutter_mixed_2_instance_2d() {
    let fixture = RootFixture { mixed: true, ..Default::default() };
    let time = Tick::from_seconds_f64(0.5);
    let direct = root_camera_scene(0, false, SceneRenderer::Advanced3D, false, true, time, fixture);
    let nested = root_camera_scene(2, false, SceneRenderer::Advanced3D, true, false, time, fixture);
    assert!(image_difference(&direct, &nested) < 1e-5, "the collapsed mixed scene follows the same root sample");
}

#[test]
fn advanced_root_shutter_animated_light_2_instance_3d() {
    let fixture = RootFixture { animated_light: true, ..Default::default() };
    let time = Tick::from_seconds_f64(0.5);
    let direct = root_camera_scene(0, false, SceneRenderer::Advanced3D, false, true, time, fixture);
    let nested = root_camera_scene(2, false, SceneRenderer::Advanced3D, true, true, time, fixture);
    assert!(image_difference(&direct, &nested) < 1e-5, "the collapsed animated_light scene follows the same root sample");
}

#[test]
fn advanced_root_shutter_mixed_2_instance_3d() {
    let fixture = RootFixture { mixed: true, ..Default::default() };
    let time = Tick::from_seconds_f64(0.5);
    let direct = root_camera_scene(0, false, SceneRenderer::Advanced3D, false, true, time, fixture);
    let nested = root_camera_scene(2, false, SceneRenderer::Advanced3D, true, true, time, fixture);
    assert!(image_difference(&direct, &nested) < 1e-5, "the collapsed mixed scene follows the same root sample");
}

#[test]
fn advanced_root_light_control_observes_shutter_samples() {
    let time = Tick::from_seconds_f64(0.5);
    let fixture = RootFixture { animated_light: true, ..Default::default() };
    let moving = root_camera_scene(0, true, SceneRenderer::Advanced3D, false, true, time, fixture);
    let held = root_camera_scene(0, true, SceneRenderer::Advanced3D, false, true, time, RootFixture { held_light: true, ..fixture });
    assert!(image_difference(&moving, &held) > 0.05, "the control must observe the light changing within the shutter");
}

fn disabled_root_blur_matches_direct(mode: SceneRenderer, levels: usize, container: bool, blur: BlurScope) {
    let fixture = RootFixture { blur, ..Default::default() };
    let time = Tick::from_seconds_f64(0.5);
    let direct = root_camera_scene(0, false, mode, false, true, time, fixture);
    let nested = root_camera_scene(levels, false, mode, true, container, time, fixture);
    assert!(image_difference(&direct, &nested) < 1e-5, "disabled root blur: {mode:?}, levels={levels}, container={container}, blur={blur:?}");
}

#[test]
fn classic_root_blur_renderoff_1_2d() {
    disabled_root_blur_matches_direct(SceneRenderer::Classic3D, 1, false, BlurScope::RenderOff);
}

#[test]
fn classic_root_blur_rootoff_1_2d() {
    disabled_root_blur_matches_direct(SceneRenderer::Classic3D, 1, false, BlurScope::RootOff);
}

#[test]
fn classic_root_blur_renderoff_1_3d() {
    disabled_root_blur_matches_direct(SceneRenderer::Classic3D, 1, true, BlurScope::RenderOff);
}

#[test]
fn classic_root_blur_rootoff_1_3d() {
    disabled_root_blur_matches_direct(SceneRenderer::Classic3D, 1, true, BlurScope::RootOff);
}

#[test]
fn classic_root_blur_renderoff_2_2d() {
    disabled_root_blur_matches_direct(SceneRenderer::Classic3D, 2, false, BlurScope::RenderOff);
}

#[test]
fn classic_root_blur_rootoff_2_2d() {
    disabled_root_blur_matches_direct(SceneRenderer::Classic3D, 2, false, BlurScope::RootOff);
}

#[test]
fn classic_root_blur_renderoff_2_3d() {
    disabled_root_blur_matches_direct(SceneRenderer::Classic3D, 2, true, BlurScope::RenderOff);
}

#[test]
fn classic_root_blur_rootoff_2_3d() {
    disabled_root_blur_matches_direct(SceneRenderer::Classic3D, 2, true, BlurScope::RootOff);
}

#[test]
fn classic_root_blur_disabled_control_changes_the_picture() {
    let time = Tick::from_seconds_f64(0.5);
    let enabled = root_camera_scene(0, false, SceneRenderer::Classic3D, false, true, time, RootFixture::default());
    let disabled = root_camera_scene(0, false, SceneRenderer::Classic3D, false, true, time, RootFixture { blur: BlurScope::RootOff, ..Default::default() });
    assert!(image_difference(&enabled, &disabled) > 0.05, "the control must observe the root blur switch");
}

#[test]
fn classic_uncollapsed_boundary_keeps_its_own_camera() {
    let time = Tick::from_seconds_f64(0.5);
    let fixture = RootFixture { collapsed: false, ..Default::default() };
    let own = root_camera_scene(2, false, SceneRenderer::Classic3D, true, false, time, fixture);
    let held_root = root_camera_scene(2, true, SceneRenderer::Classic3D, true, false, time, fixture);
    assert!(own.data.iter().filter(|p| p[3] > 0.5).count() > 100, "the nested scene must be visible");
    assert!(image_difference(&own, &held_root) < 1e-5, "an uncollapsed 2D source keeps its own camera boundary");
}

#[test]
fn advanced_root_blur_renderoff_1_2d() {
    disabled_root_blur_matches_direct(SceneRenderer::Advanced3D, 1, false, BlurScope::RenderOff);
}

#[test]
fn advanced_root_blur_rootoff_1_2d() {
    disabled_root_blur_matches_direct(SceneRenderer::Advanced3D, 1, false, BlurScope::RootOff);
}

#[test]
fn advanced_root_blur_renderoff_1_3d() {
    disabled_root_blur_matches_direct(SceneRenderer::Advanced3D, 1, true, BlurScope::RenderOff);
}

#[test]
fn advanced_root_blur_rootoff_1_3d() {
    disabled_root_blur_matches_direct(SceneRenderer::Advanced3D, 1, true, BlurScope::RootOff);
}

#[test]
fn advanced_root_blur_renderoff_2_2d() {
    disabled_root_blur_matches_direct(SceneRenderer::Advanced3D, 2, false, BlurScope::RenderOff);
}

#[test]
fn advanced_root_blur_rootoff_2_2d() {
    disabled_root_blur_matches_direct(SceneRenderer::Advanced3D, 2, false, BlurScope::RootOff);
}

#[test]
fn advanced_root_blur_renderoff_2_3d() {
    disabled_root_blur_matches_direct(SceneRenderer::Advanced3D, 2, true, BlurScope::RenderOff);
}

#[test]
fn advanced_root_blur_rootoff_2_3d() {
    disabled_root_blur_matches_direct(SceneRenderer::Advanced3D, 2, true, BlurScope::RootOff);
}

#[test]
fn advanced_root_blur_disabled_control_changes_the_picture() {
    let time = Tick::from_seconds_f64(0.5);
    let enabled = root_camera_scene(0, false, SceneRenderer::Advanced3D, false, true, time, RootFixture::default());
    let disabled = root_camera_scene(0, false, SceneRenderer::Advanced3D, false, true, time, RootFixture { blur: BlurScope::RootOff, ..Default::default() });
    assert!(image_difference(&enabled, &disabled) > 0.05, "the control must observe the root blur switch");
}

#[test]
fn advanced_uncollapsed_boundary_keeps_its_own_camera() {
    let time = Tick::from_seconds_f64(0.5);
    let fixture = RootFixture { collapsed: false, ..Default::default() };
    let own = root_camera_scene(2, false, SceneRenderer::Advanced3D, true, false, time, fixture);
    let held_root = root_camera_scene(2, true, SceneRenderer::Advanced3D, true, false, time, fixture);
    assert!(own.data.iter().filter(|p| p[3] > 0.5).count() > 100, "the nested scene must be visible");
    assert!(image_difference(&own, &held_root) < 1e-5, "an uncollapsed 2D source keeps its own camera boundary");
}
