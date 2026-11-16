//! Decoded scene inputs live once through preparation and both eye renders.
use std::collections::HashMap;
use std::hash::Hash;
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
};

use crate::{FloatCurve, FootageSource, Image};
use effectcraft_project::{Footage, ItemId};
use effectcraft_time::Tick;

type SourceKey = (ItemId, String);
type FrameKey = (SourceKey, Tick, Option<(u32, u32)>, FloatCurve);
type ByteKey = (SourceKey, Tick, FloatCurve);
type VectorKey = (SourceKey, u64);
type AudioKey = (SourceKey, Tick, usize, u32);
type Frames = HashMap<FrameKey, Option<Arc<Image>>>;
type Bytes = HashMap<ByteKey, Option<Arc<crate::disk_cache::Frame8>>>;
type Vectors = HashMap<VectorKey, Option<Arc<Image>>>;
type Models = HashMap<SourceKey, Option<Arc<effectcraft_model::Model>>>;
type Aux = HashMap<(SourceKey, Tick), Option<Arc<effectcraft_raster::AuxChannels>>>;
type Audio = HashMap<AudioKey, Option<Vec<f32>>>;

pub(super) struct FrozenFootage<'a> {
    inner: &'a dyn FootageSource,
    failed: AtomicBool,
    missing: Mutex<Option<String>>,
    frames: Mutex<Frames>,
    bytes: Mutex<Bytes>,
    vectors: Mutex<Vectors>,
    models: Mutex<Models>,
    aux: Mutex<Aux>,
    audio: Mutex<Audio>,
}

impl<'a> FrozenFootage<'a> {
    pub(super) fn new(inner: &'a dyn FootageSource) -> Self {
        Self {
            inner,
            failed: AtomicBool::new(false),
            missing: Mutex::new(None),
            frames: Mutex::default(),
            bytes: Mutex::default(),
            vectors: Mutex::default(),
            models: Mutex::default(),
            aux: Mutex::default(),
            audio: Mutex::default(),
        }
    }
    fn memo<K: Eq + Hash, V: Clone + Default>(&self, cache: &Mutex<HashMap<K, V>>, key: K, read: impl FnOnce() -> V) -> V {
        let Ok(mut values) = cache.lock() else {
            self.failed.store(true, Ordering::Relaxed);
            return V::default();
        };
        values.entry(key).or_insert_with(read).clone()
    }
    fn missing(&self, footage: &Footage, kind: &str) {
        if let Ok(mut missing) = self.missing.lock() {
            missing.get_or_insert_with(|| self.inner.problem(footage).unwrap_or_else(|| format!("the {kind} scene input {:?} is unavailable", footage.path)));
        } else {
            self.failed.store(true, Ordering::Relaxed);
        }
    }
    pub(super) fn check(&self) -> Result<(), String> {
        if self.failed.load(Ordering::Relaxed) {
            return Err("the decoded scene resource snapshot was interrupted".into());
        }
        if self.inner.called_off() {
            return Err("the stereo scene frame was cancelled before the pair completed".into());
        }
        let missing = self.missing.lock().map_err(|_| "the decoded scene resource snapshot was interrupted".to_string())?;
        if let Some(error) = &*missing {
            return Err(error.clone());
        }
        Ok(())
    }
    fn key(item: ItemId, footage: &Footage) -> SourceKey {
        // An enabled proxy and the original can share an item id, but not their interpretation.
        (item, format!("{footage:?}"))
    }
    pub(super) fn counts(&self) -> [usize; 6] {
        [
            self.frames.lock().map_or(0, |values| values.len()),
            self.bytes.lock().map_or(0, |values| values.len()),
            self.vectors.lock().map_or(0, |values| values.len()),
            self.models.lock().map_or(0, |values| values.len()),
            self.aux.lock().map_or(0, |values| values.len()),
            self.audio.lock().map_or(0, |values| values.len()),
        ]
    }
}

impl FootageSource for FrozenFootage<'_> {
    fn frame(&self, item: ItemId, footage: &Footage, time: Tick) -> Option<Arc<Image>> {
        self.frame_with(item, footage, time, None, FloatCurve::Srgb)
    }
    fn frame_at_size(&self, item: ItemId, footage: &Footage, time: Tick, width: u32, height: u32) -> Option<Arc<Image>> {
        self.frame_with(item, footage, time, Some((width, height)), FloatCurve::Srgb)
    }
    fn frame_with(&self, item: ItemId, footage: &Footage, time: Tick, size: Option<(u32, u32)>, curve: FloatCurve) -> Option<Arc<Image>> {
        let image = self.memo(&self.frames, (Self::key(item, footage), time, size, curve), || self.inner.frame_with(item, footage, time, size, curve));
        if image.is_none() {
            self.missing(footage, "image");
        }
        image
    }
    fn frame8(&self, item: ItemId, footage: &Footage, time: Tick, curve: FloatCurve) -> Option<Arc<crate::disk_cache::Frame8>> {
        self.memo(&self.bytes, (Self::key(item, footage), time, curve), || self.inner.frame8(item, footage, time, curve))
    }
    fn vector_frame(&self, item: ItemId, footage: &Footage, scale: f64) -> Option<Arc<Image>> {
        self.memo(&self.vectors, (Self::key(item, footage), scale.to_bits()), || self.inner.vector_frame(item, footage, scale))
    }
    fn model(&self, item: ItemId, footage: &Footage) -> Option<Arc<effectcraft_model::Model>> {
        let model = self.memo(&self.models, Self::key(item, footage), || self.inner.model(item, footage));
        if model.is_none() {
            self.missing(footage, "model");
        }
        model
    }
    fn aux(&self, item: ItemId, footage: &Footage, time: Tick) -> Option<Arc<effectcraft_raster::AuxChannels>> {
        self.memo(&self.aux, (Self::key(item, footage), time), || self.inner.aux(item, footage, time))
    }
    fn audio(&self, item: ItemId, footage: &Footage, time: Tick, frames: usize, rate: u32) -> Option<Vec<f32>> {
        self.memo(&self.audio, (Self::key(item, footage), time, frames, rate), || self.inner.audio(item, footage, time, frames, rate))
    }
    fn called_off(&self) -> bool {
        self.failed.load(Ordering::Relaxed) || self.inner.called_off()
    }
    fn cancellable(&self, cancel: &Arc<AtomicBool>, read: &mut dyn FnMut()) {
        self.inner.cancellable(cancel, read);
    }
    fn recording(&self, reads: &crate::reads::ReadLog, read: &mut dyn FnMut()) {
        self.inner.recording(reads, read);
    }
    fn problem(&self, footage: &Footage) -> Option<String> {
        self.inner.problem(footage)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicUsize;
    #[derive(Default)]
    struct Source {
        frames: AtomicUsize,
        bytes: AtomicUsize,
        audio: AtomicUsize,
        models: AtomicUsize,
    }
    impl FootageSource for Source {
        fn frame(&self, item: ItemId, footage: &Footage, time: Tick) -> Option<Arc<Image>> {
            self.frame_with(item, footage, time, None, FloatCurve::Srgb)
        }
        fn frame_with(&self, _item: ItemId, footage: &Footage, _time: Tick, size: Option<(u32, u32)>, curve: FloatCurve) -> Option<Arc<Image>> {
            self.frames.fetch_add(1, Ordering::SeqCst);
            let (w, h) = size.unwrap_or((4, 2));
            let mut image = Image::new(w, h);
            let red = if curve == FloatCurve::Linear { 0.75 } else { 0.25 };
            image.data.fill(if footage.path == "proxy.png" { [0.0, 1.0, 0.0, 1.0] } else { [red, 0.0, 0.0, 1.0] });
            Some(Arc::new(image))
        }
        fn frame8(&self, _item: ItemId, _footage: &Footage, _time: Tick, _curve: FloatCurve) -> Option<Arc<crate::disk_cache::Frame8>> {
            self.bytes.fetch_add(1, Ordering::SeqCst);
            None
        }
        fn audio(&self, _item: ItemId, _footage: &Footage, _time: Tick, _frames: usize, _rate: u32) -> Option<Vec<f32>> {
            self.audio.fetch_add(1, Ordering::SeqCst);
            None
        }
        fn model(&self, _item: ItemId, _footage: &Footage) -> Option<Arc<effectcraft_model::Model>> {
            self.models.fetch_add(1, Ordering::SeqCst);
            None
        }
    }
    fn footage(path: &str) -> Footage {
        Footage { path: path.into(), width: 4, height: 2, has_video: true, ..Default::default() }
    }

    #[test]
    fn proxy_interpretation_curve_and_decode_size_remain_distinct() {
        let source = Source::default();
        let snapshot = FrozenFootage::new(&source);
        let original = footage("original.png");
        let proxy = footage("proxy.png");
        let a = snapshot.frame(ItemId(1), &original, Tick::ZERO).unwrap();
        assert!(Arc::ptr_eq(&a, &snapshot.frame(ItemId(1), &original, Tick::ZERO).unwrap()));
        let b = snapshot.frame(ItemId(1), &proxy, Tick::ZERO).unwrap();
        assert_ne!(a.data, b.data);
        let c = snapshot.frame_with(ItemId(1), &original, Tick::ZERO, None, FloatCurve::Linear).unwrap();
        assert_ne!(a.data, c.data);
        let d = snapshot.frame_at_size(ItemId(1), &original, Tick::ZERO, 2, 1).unwrap();
        assert_eq!((d.width, d.height), (2, 1));
        assert_eq!(source.frames.load(Ordering::SeqCst), 4);
        assert!(snapshot.check().is_ok());
    }
    #[test]
    fn optional_unavailable_queries_are_frozen_without_losing_a_valid_image() {
        let source = Source::default();
        let snapshot = FrozenFootage::new(&source);
        let input = footage("original.png");
        for _ in 0..2 {
            assert!(snapshot.frame8(ItemId(1), &input, Tick::ZERO, FloatCurve::Srgb).is_none());
            assert!(snapshot.audio(ItemId(1), &input, Tick::ZERO, 1024, 48000).is_none());
            assert!(snapshot.frame(ItemId(1), &input, Tick::ZERO).is_some());
        }
        assert_eq!(source.bytes.load(Ordering::SeqCst), 1);
        assert_eq!(source.audio.load(Ordering::SeqCst), 1);
        assert_eq!(source.frames.load(Ordering::SeqCst), 1);
        assert!(snapshot.check().is_ok());
    }
    #[test]
    fn a_required_model_cannot_resolve_between_the_eyes() {
        let source = Source::default();
        let snapshot = FrozenFootage::new(&source);
        let input = footage("missing-model.glb");
        for _ in 0..2 {
            assert!(snapshot.model(ItemId(1), &input).is_none());
        }
        assert_eq!(source.models.load(Ordering::SeqCst), 1);
        assert!(snapshot.check().unwrap_err().contains("model"));
    }
    #[test]
    fn cancellation_still_reaches_a_snapshot_with_cached_inputs() {
        let source = Source::default();
        let cancelled = Arc::new(AtomicBool::new(false));
        let cancellable = crate::CancellableFootage { inner: &source, cancel: cancelled.clone() };
        let snapshot = FrozenFootage::new(&cancellable);
        let input = footage("original.png");
        assert!(snapshot.frame(ItemId(1), &input, Tick::ZERO).is_some());
        assert!(snapshot.check().is_ok());
        cancelled.store(true, Ordering::SeqCst);
        assert!(snapshot.check().unwrap_err().contains("cancelled"));
    }
}
