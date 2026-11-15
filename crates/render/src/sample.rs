//! One root shutter clock through the frame-selected collapsed instance chain.

use crate::opts::{Collapse3d, MAX_THROUGH};
use crate::pipeline::collapsed_instance;
use crate::{Comp, EvalCtx, ItemId, LayerSource, Mat3, Mat4, MediaOverride, PropertyOverride, Renderer, Tick, three_d};

struct Instance<'a> {
    comp_id: ItemId,
    comp: &'a Comp,
    time: Tick,
    world: Mat4,
    outer: Mat3,
    properties: Vec<PropertyOverride>,
    media: Vec<MediaOverride>,
}

impl Renderer<'_> {
    /// Consume sampled override vectors while their complete ancestor chain remains alive.
    pub(crate) fn with_root_sample<R>(&self, frame: &EvalCtx, offset: f64, draw: impl FnOnce(&Renderer<'_>, &EvalCtx<'_>) -> R) -> Option<R> {
        let shift = Tick::from_seconds_f64(offset);
        let Some(collapsed) = self.collapse3d else {
            return Some(draw(self, &frame.at(frame.time + shift)));
        };
        if self.through_lost {
            return None;
        }
        let root = collapsed.frame.at(collapsed.frame.time + shift);
        let camera = collapsed.view.unwrap_or_else(|| three_d::camera::frame_camera(&root, collapsed.frame.time));
        let mut instances: Vec<Instance<'_>> = Vec::with_capacity(MAX_THROUGH);
        for link in self.through.iter().map_while(Option::as_ref) {
            let parent = match instances.last() {
                Some(instance) => EvalCtx {
                    comp_id: instance.comp_id,
                    comp: instance.comp,
                    time: instance.time,
                    property_overrides: &instance.properties,
                    media_overrides: &instance.media,
                    ..root
                },
                None => root,
            };
            if parent.comp_id != link.comp_id {
                return None;
            }
            let LayerSource::Comp { item } = link.layer.source else { return None };
            let child = self.project.comp(item)?;
            let transform = if link.blur { parent } else { link.context };
            let (properties, media) = collapsed_instance(&transform, link.layer, item)?;
            let world = instances.last().map_or(Mat4::IDENTITY, |instance| instance.world);
            let outer = instances.last().map_or(Mat3::IDENTITY, |instance| instance.outer);
            let (local, _) = transform.layer_to_comp(link.layer);
            let instance = Instance {
                comp_id: item,
                comp: child,
                time: parent.nested_time(link.layer),
                world: world * transform.world_matrix(link.layer),
                outer: outer * local,
                properties,
                media,
            };
            instances.push(instance);
        }
        let instance = instances.last()?;
        if instance.comp_id != frame.comp_id {
            return None;
        }
        let sample = EvalCtx {
            comp_id: instance.comp_id,
            comp: instance.comp,
            time: instance.time,
            property_overrides: &instance.properties,
            media_overrides: &instance.media,
            ..root
        };
        let sampled = Renderer {
            outer: Some(instance.outer),
            collapse3d: Some(Collapse3d { world: instance.world, cam: camera, parent: root, frame: collapsed.frame, view: collapsed.view }),
            ..*self
        };
        Some(draw(&sampled, &sample))
    }
}
