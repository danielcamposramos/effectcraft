# Composition camera pairs

`Renderer::stereo_frame` renders two CPU views of one borrowed project, composition and exact frame time.
The width is per eye; the document aspect ratio determines its height, independently of viewer magnification, playback adaptation or the desktop output size.
The pair API currently limits each eye to sixteen million pixels so preparation, two float frames and their packed picture remain bounded.
This is a new pair work guard, using the same numeric bound as upstream’s individual solid/effect buffers; upstream does not apply it to an entire ordinary composition frame.
The caller supplies the compositor's finite nonnegative native-pixel depth budget.

Preparation observes the actual Classic planes and Advanced world vertices before viewport culling, including collapsed instances and every evaluated root shutter sample.
The active root camera is selected at the requested frame and evaluated at each sample; an explicit existing view override remains held.
The evaluated two-node point of interest supplies the zero plane; one-node cameras retain their image plane and ignore inactive Point of Interest properties.
An uncollapsed composition remains an authored flat source with its own internal camera, even if it contains collapsed instances; its internal geometry never changes the root envelope.
One normalized depth envelope covers the full scene and shutter.
`craft-stereo-camera` supplies the parallel eye offset and off-axis correction, adapted to EffectCraft's f64 composition-pixel coordinates and positive-forward camera convention.
Environment backgrounds reserve infinity.
Flat art retains its ordinary projection.

Each eye owns its renderer, processed buffers and effect state; no shared mutable accelerator or processed-layer cache is accepted.
One per-pair decoded-input snapshot retains float and byte frames, vector rasters, models, auxiliary channels and audio queries through preparation and both eyes.
The key includes the complete footage interpretation and proxy descriptor, exact source time, requested decode size and float curve.
Optional unavailable results are frozen too; a required image or model that cannot resolve returns the source's error instead of publishing a blank world.
Cancellation still reaches the underlying source, and the snapshot is discarded after the pair.
The snapshot retains immutable decoded inputs; processed effects and histories remain private to each eye.
Both eyes must report exactly the same evaluated central cameras, root sample times and baselines.
Cancellation, malformed cameras, interrupted root instance chains and invalid depth feedback return an error, never a partial pair.
The ordinary renderer and its acceleration routes are unchanged.

Camera-space effects that privately rasterize internal 3D geometry currently expose only a projection/light through `EffectHost::comp_scene`, with no geometry depth envelope.
The stereo API explicitly refuses these effects; their ordinary render remains available.
To support them, their owning effects engines must emit their actual geometry bounds and consume the evaluated off-axis eye camera, preserving private simulation/history state.
Copying or shifting their flattened result does not provide a second view.

The original procedural test scenes cover Classic planes, Advanced mesh/card runs, direct and collapsed instances, moving root cameras with motion blur, two native widths, the independent disparity equation, vertical alignment, reversed-eye controls, exact flat output and exact zero-budget output.
They are original work by Daniel Campos Ramos under the project licence.
The camera pair renderer has no desktop dependency; declaration and presentation belong to a frontend adapter.


## Native companion

The native editor's View menu offers Enable 3D, Render resolution per eye and Display Settings.
The document stage supplies the initial native sampling; a chosen width keeps its authored aspect, with height shown alongside it.
Document pixel aspect is declared as metadata without resizing either eye.
Transparent pixels are composited over the authored composition background, exactly as ordinary output.
The desktop receives one full side-by-side buffer, left eye first, and scales it once to the companion window.
The composition is the program's own world, so this view has no eye-swap control.

One cancellable worker borrows an immutable project snapshot and publishes only a completed pair.
Paused seeking, changed project state, quality or depth feedback discard obsolete work; playback may drop complete old frames while keeping the two eye times together.
The native window receives the compositor's live depth budget and rerenders when it changes.
While waiting for initial feedback the status says so; after the first pair maps, missing feedback for three seconds closes the companion with an explicit error.
Display Settings opens the desktop's `kcm_kscreen` module, with a visible error if it cannot start.
The existing WASM editor remains available; the native companion reports that it requires the desktop app.

The added 3D phrases cover ten languages including Spanish and Korean.
Korean is explicitly offered as a 3D-only catalog; this change does not claim a complete Korean menu translation.
