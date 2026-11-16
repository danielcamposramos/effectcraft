//! Evaluated composition cameras; the common core sees only camera-relative ratios.
use std::ops::Deref;

use craft_stereo_camera::{Budget, Camera, DepthRange, ProjectionKind};
use effectcraft_geom::{Mat4, Vec2, Vec3, vec2, vec3};
use glam::{Mat4 as CoreMat4, Vec3 as CoreVec3};

use super::camera::CameraState;

/// One physical view and its off-axis correction in composition-pixel units.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ViewCamera {
    state: CameraState,
    centre_x: f64,
    depth_x: f64,
}

impl Deref for ViewCamera {
    type Target = CameraState;
    fn deref(&self) -> &Self::Target {
        &self.state
    }
}

impl ViewCamera {
    pub fn mono(state: CameraState) -> Self {
        Self { state, centre_x: 0.0, depth_x: 0.0 }
    }

    pub fn projection(&self, width: f64, height: f64) -> Mat4 {
        if self.centre_x == 0.0 && self.depth_x == 0.0 {
            return self.state.projection(width, height);
        }
        let mut projection = self.state.projection(width, height);
        let w = projection.0[3];
        let depth = self.view.0[2];
        for i in 0..4 {
            projection.0[0][i] += self.centre_x * w[i] + self.depth_x * depth[i];
        }
        projection
    }

    pub fn project(&self, width: f64, height: f64, point: Vec3) -> Option<Vec2> {
        let projected = self.state.project(width, height, point)?;
        if self.centre_x == 0.0 && self.depth_x == 0.0 {
            return Some(projected);
        }
        Some(vec2(projected.x + self.centre_x + self.depth_x * self.state.depth(point), projected.y))
    }

    pub(crate) fn sky_forward(&self) -> Vec3 {
        if self.ortho { self.forward() } else { self.forward() - self.right() * (self.centre_x / self.zoom) }
    }

    /// Add the identical correction to Advanced 3D's existing reversed-depth clip matrix.
    pub(crate) fn clip_correction(&self, scale: f64, output_width: f64) -> (f64, f64) {
        (2.0 * scale * self.centre_x / output_width, 2.0 * scale * self.depth_x / output_width)
    }
}

/// A pair evaluated at one actual shutter time, left first.
#[derive(Clone, Copy, Debug)]
pub struct CameraViews {
    pub eyes: [ViewCamera; 2],
    pub baseline: f64,
}

fn rigid_camera(camera: CameraState) -> bool {
    let axes = [camera.right(), camera.down(), camera.forward()];
    let lengths = axes.iter().all(|axis| (axis.dot(*axis) - 1.0).abs() < 1e-9);
    let parallel = axes[0].dot(axes[1]).abs() < 1e-9 && axes[0].dot(axes[2]).abs() < 1e-9 && axes[1].dot(axes[2]).abs() < 1e-9;
    let handed = (axes[0].cross(axes[1]) - axes[2]).length() < 1e-9;
    let affine = camera.view.0[3] == [0.0, 0.0, 0.0, 1.0];
    let origin = camera.view.apply(camera.eye).length();
    let tolerance = 1e-9 + camera.eye.length() * f64::EPSILON * 16.0;
    lengths && parallel && handed && affine && origin.is_finite() && origin <= tolerance
}

/// Keep the f64 world origin and physical distances out of the f32 camera core.
/// `zero_depth` comes from the evaluated existing point of interest or image plane.
/// `depths` covers the composition at this sample, not just one layer run.
pub fn camera_views(
    central: CameraState,
    comp: [f64; 2],
    zero_depth: f64,
    depths: [f64; 2],
    native_width: u32,
    budget: Budget,
    infinity_background: bool,
) -> Result<CameraViews, String> {
    let [comp_width, comp_height] = comp;
    if ![comp_width, comp_height, zero_depth, central.zoom].into_iter().all(|v| v.is_finite() && v > 0.0)
        || !central.view.0.iter().flatten().all(|v| v.is_finite())
        || ![central.eye.x, central.eye.y, central.eye.z].into_iter().all(f64::is_finite)
        || !depths.into_iter().all(|v| v.is_finite() && v > 0.0)
        || depths[0] > depths[1]
    {
        return Err("the evaluated composition camera or depth range is not finite and positive".into());
    }
    if !rigid_camera(central) {
        return Err("the evaluated composition view must be a finite rigid transform consistent with its eye".into());
    }
    let unit = if central.ortho { zero_depth } else { 1.0 };
    let fx = (2.0 * central.zoom * unit / comp_width) as f32;
    let fy = (2.0 * central.zoom * unit / comp_height) as f32;
    // Only baseline derivation uses this canonical projection. The renderer keeps its own
    // f64 projection, near clipping and composition-pixel coordinate conventions.
    let projection = if central.ortho {
        CoreMat4::from_cols_array(&[fx, 0.0, 0.0, 0.0, 0.0, fy, 0.0, 0.0, 0.0, 0.0, -1.0, 0.0, 0.0, 0.0, 0.0, 1.0])
    } else {
        CoreMat4::from_cols_array(&[fx, 0.0, 0.0, 0.0, 0.0, fy, 0.0, 0.0, 0.0, 0.0, -1.0, -1.0, 0.0, 0.0, -1.0, 0.0])
    };
    let pair = Camera {
        eye: CoreVec3::ZERO,
        view: CoreMat4::IDENTITY,
        projection,
        infinity_projection_x: if infinity_background { (2.0 * central.zoom / comp_width) as f32 } else { 0.0 },
        kind: if central.ortho { ProjectionKind::Orthographic } else { ProjectionKind::Perspective },
        zero_depth: 1.0,
        depths: DepthRange { near: (depths[0] / zero_depth) as f32, far: (depths[1] / zero_depth) as f32 },
        width: native_width,
        budget,
    }
    .pair()
    .map_err(|e| e.to_string())?;
    if pair.baseline == 0.0 {
        return Ok(CameraViews { eyes: [ViewCamera::mono(central); 2], baseline: 0.0 });
    }
    let baseline = f64::from(pair.baseline) * zero_depth;
    if !baseline.is_finite() || baseline == 0.0 {
        return Err("the physical composition camera baseline is outside the finite range".into());
    }
    let mut eyes = [ViewCamera::mono(central); 2];
    for (output, canonical) in eyes.iter_mut().zip(pair.eyes) {
        let offset = f64::from(canonical.eye.x) * zero_depth;
        let correction = central.zoom * offset / zero_depth;
        output.state.eye = central.eye + central.right() * offset;
        output.state.view = Mat4::translate(vec3(-offset, 0.0, 0.0)) * central.view;
        if central.ortho {
            output.depth_x = correction;
        } else {
            output.centre_x = correction;
        }
        if !output.state.view.0.iter().flatten().all(|v| v.is_finite())
            || ![output.eye.x, output.eye.y, output.eye.z, output.centre_x, output.depth_x].into_iter().all(f64::is_finite)
        {
            return Err("the physical composition eye projection is outside the finite range".into());
        }
    }
    Ok(CameraViews { eyes, baseline })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::three_d::camera::basis_view;

    fn camera(ortho: bool, translated: bool) -> CameraState {
        let eye = if translated { vec3(12345.0, -6789.0, 987.0) } else { vec3(100.0, 50.0, -250.0) };
        let forward = if translated { vec3(0.3, -0.2, 1.0).normalize() } else { vec3(0.0, 0.0, 1.0) };
        CameraState { eye, view: basis_view(eye, forward, vec3(0.0, 1.0, 0.0)), zoom: if ortho { 1.25 } else { 250.0 }, ortho, dof: None }
    }

    #[test]
    fn composition_cameras_match_the_independent_disparity_equation() {
        for ortho in [false, true] {
            for translated in [false, true] {
                let central = camera(ortho, translated);
                for width in [200, 640, 1280] {
                    let pair = camera_views(central, [200.0, 100.0], 250.0, [200.0, 300.0], width, Budget { popped: 12.0, sunk: 8.0 }, false).unwrap();
                    assert!(pair.baseline > 0.0);
                    assert_eq!(pair.eyes[0].forward(), pair.eyes[1].forward());
                    assert_eq!(pair.eyes[0].down(), pair.eyes[1].down());
                    for z in [200.0, 225.0, 250.0, 275.0, 300.0] {
                        for x in [-23.0, 0.0, 31.0] {
                            for y in [-11.0, 0.0, 17.0] {
                                let point = central.eye + central.right() * x + central.down() * y + central.forward() * z;
                                let [left, right] = pair.eyes.map(|eye| eye.project(200.0, 100.0, point).unwrap());
                                let expected = pair.baseline * central.zoom * if ortho { 1.0 - z / 250.0 } else { 1.0 / z - 1.0 / 250.0 };
                                assert!((left.x - right.x - expected).abs() < 1e-8, "{ortho} {translated} {width} {z}");
                                assert!((left.y - right.y).abs() < 1e-10);
                                for (eye, projected) in pair.eyes.into_iter().zip([left, right]) {
                                    let actual = eye.projection(200.0, 100.0).apply(point);
                                    assert!((actual.x - projected.x).abs() < 1e-8);
                                    assert!((actual.y - projected.y).abs() < 1e-8);
                                }
                                let native_disparity = (left.x - right.x) * f64::from(width) / 200.0;
                                assert!((-8.0001..=12.0001).contains(&native_disparity));
                                if z == 250.0 {
                                    assert!((left.x - right.x).abs() < 1e-8);
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn zero_budget_retains_the_exact_central_projection() {
        for ortho in [false, true] {
            let central = camera(ortho, true);
            let pair = camera_views(central, [200.0, 100.0], 250.0, [200.0, 300.0], 640, Budget { popped: 0.0, sunk: 0.0 }, false).unwrap();
            assert_eq!(pair.baseline, 0.0);
            for eye in pair.eyes {
                assert_eq!(eye.state, central);
                assert_eq!(eye.projection(200.0, 100.0), central.projection(200.0, 100.0));
                assert_eq!(eye.clip_correction(3.2, 640.0), (0.0, 0.0));
            }
        }
    }

    #[test]
    fn invalid_camera_or_depth_is_reported() {
        let central = camera(false, false);
        for depths in [[f64::NAN, 300.0], [0.0, 300.0], [300.0, 200.0], [200.0, f64::INFINITY]] {
            assert!(camera_views(central, [200.0, 100.0], 250.0, depths, 640, Budget { popped: 12.0, sunk: 8.0 }, false).is_err());
        }
        for zero in [0.0, -1.0, f64::NAN, f64::INFINITY] {
            assert!(camera_views(central, [200.0, 100.0], zero, [200.0, 300.0], 640, Budget { popped: 12.0, sunk: 8.0 }, false).is_err());
        }
    }

    #[test]
    fn infinity_reserves_the_sunk_budget() {
        let central = camera(false, false);
        let pair = camera_views(central, [200.0, 100.0], 250.0, [200.0, 300.0], 640, Budget { popped: 12.0, sunk: 8.0 }, true).unwrap();
        let infinity = pair.baseline * central.zoom * 640.0 / 200.0 / 250.0;
        assert!(infinity <= 8.0001);
    }

    #[test]
    fn a_finite_malformed_view_is_reported_before_eyes_are_created() {
        let central = camera(false, false);
        for view in [
            Mat4([[0.0; 4]; 4]),
            Mat4([[2.0, 0.0, 0.0, -100.0], [0.0, 1.0, 0.0, -50.0], [0.0, 0.0, 1.0, 250.0], [0.0, 0.0, 0.0, 1.0]]),
            Mat4([[1.0, 0.3, 0.0, -100.0], [0.0, 1.0, 0.0, -50.0], [0.0, 0.0, 1.0, 250.0], [0.0, 0.0, 0.0, 1.0]]),
        ] {
            let malformed = CameraState { view, ..central };
            assert!(camera_views(malformed, [200.0, 100.0], 250.0, [200.0, 300.0], 640, Budget { popped: 12.0, sunk: 8.0 }, false).is_err());
        }
    }
}
