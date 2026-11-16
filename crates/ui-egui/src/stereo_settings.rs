//! Session view quality; default sampling is the document stage, never the desktop.
#[derive(Clone, Copy, Debug, Default, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct StereoSettings {
    pub enabled: bool,
    /// Width in native samples per eye; height follows the document aspect ratio.
    pub width: Option<u32>,
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use crate::{EffectcraftApp, menus};
    use serde_json::json;
    #[test]
    fn stereo_commands_preserve_the_document_and_reject_invalid_quality_atomically() {
        let mut session = effectcraft_engine::Session::default();
        session.execute("comp.new", json!({"width":320,"height":180})).unwrap();
        let mut app = EffectcraftApp::new(session);
        let ctx = egui::Context::default();
        assert!(!app.ui.stereo.enabled);
        assert_eq!(app.ui.stereo.width, None);
        menus::invoke(&mut app, &ctx, "view.stereo", json!({"enabled":true})).unwrap();
        assert!(app.ui.stereo.enabled);
        menus::invoke(&mut app, &ctx, "view.stereoResolution", json!({"width":640})).unwrap();
        assert_eq!(app.ui.stereo.width, Some(640));
        for width in [json!(0), json!(-1), json!(1.5), json!("640"), json!(u64::MAX)] {
            assert!(menus::invoke(&mut app, &ctx, "view.stereoResolution", json!({"width":width})).is_err());
            assert_eq!(app.ui.stereo.width, Some(640));
        }
        assert!(menus::invoke(&mut app, &ctx, "view.stereo", json!({"enabled":"yes"})).is_err());
        assert!(app.ui.stereo.enabled);
        menus::invoke(&mut app, &ctx, "view.stereoResolution", json!({"document":true})).unwrap();
        assert_eq!(app.ui.stereo.width, None);
        menus::invoke(&mut app, &ctx, "view.stereo", json!({"enabled":false})).unwrap();
        assert!(!app.ui.stereo.enabled);
        let comp = app.session.active_comp().unwrap();
        assert_eq!((comp.width, comp.height), (320, 180));
    }
    #[test]
    fn the_three_native_controls_have_their_own_korean_labels() {
        let mut session = effectcraft_engine::Session::default();
        session.execute("comp.new", json!({"width":320,"height":180})).unwrap();
        session.execute("prefs.set", json!({"key":"general.language","value":"ko"})).unwrap();
        let app = EffectcraftApp::new(session);
        let rows = menus::menu_items(&app);
        for (id, expected) in [("view.stereo", "3D 활성화"), ("view.stereoResolution", "눈별 렌더링 해상도"), ("view.displaySettings", "디스플레이 설정")]
        {
            let item = rows.iter().find(|row| row.id == id).unwrap();
            assert_eq!(item.label, expected);
            assert!(item.enabled);
        }
    }
}
