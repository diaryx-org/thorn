---
title: thorn
nav_title: thorn
nav_order: 45
description: thorn — a drawing editor over twig's SVG. Boxes, arrows, labels and ink, in a file that opens anywhere and is edited losslessly.
audience: public
part_of: '[thorn](/README.md)'
id: rr5nnsf
---
<section class="pj-head">
  <div class="wrap">
    <p><a class="crumb" href="../about/#projects">diaryx.org / projects /</a></p>
    <div class="pj-title" style="margin-top: 1rem">
      <h1>thorn</h1>
      <span class="pj-tags">
        <span class="tag-chip">Rust · Swift</span>
        <span class="tag-chip">MIT / Apache-2.0</span>
      </span>
    </div>
    <p class="pj-tagline">
      A drawing editor over twig's SVG.
    </p>
  </div>
</section>

<section class="pj-main">
<div class="wrap pj-layout reveal">
<div class="pj-body">

The drawings Excalidraw and tldraw make — a box, an arrow between two
boxes, a label, a freehand line — in a file that is an SVG and nothing
else. It opens in a browser, in a file manager's preview, on a git
forge, and in `grep`; the editor edits it losslessly through twig's
tree, so a comment, a `<style>` block or an attribute it has never
heard of survives every gesture exactly.

- **The file is the document.** No JSON scene, no cache beside the SVG. Shapes are elements, geometry is attributes, z-order is sibling order, and what the editor needs beyond SVG rides in `data-` attributes a viewer ignores.
- **A profile, written down and tested.** What a Diaryx drawing SVG is — the marker, an id on every shape, the number format that makes a re-export byte-stable — is a public page and a `check` command, not a Swift file no one can read.
- **One gesture, one splice, one undo step.** Undo is twig's; the editor keeps no second history.
- **One core, every platform.** A Rust core, a UniFFI binding for the Mac and iOS, and the browser later through wasm.

## Where it fits

thorn is how [Diaryx](id:org/80k72t9) draws. It edits through
[twig](id:twig/wxmq0ww)'s tree and displays through resvg-swift; a
drawing in an entry is an image [leaf](id:leaf/6h2bs8f) shows, and
editing it opens the drawing as its own document.

## Status

Scaffolded and not yet released: the profile, the shape model, and the
gestures — add, delete, move, resize, reorder, undo — exist and are
tested, and a canvas for the Mac and iOS edits a file live.

</div>
<aside class="pj-aside">
<div class="install">
<span class="install-head">Install</span>
<div class="cmd">cargo add thorn-svg-core <small>Rust</small></div>
</div>
<div class="facts">
<div class="row"><span class="k">Language</span><span class="v">Rust · Swift</span></div>
<div class="row"><span class="k">Built on</span><span class="v"><a href="../twig/index.md">twig</a></span></div>
<div class="row"><span class="k">Used by</span><span class="v"><a href="../index.html">Diaryx</a> — drawings</span></div>
<div class="row"><span class="k">Source</span><span class="v"><a href="https://github.com/diaryx-org/thorn">github.com/diaryx-org/thorn</a></span></div>
<div class="row"><span class="k">License</span><span class="v">MIT or Apache-2.0</span></div>
</div>
</aside>
</div>
</section>
