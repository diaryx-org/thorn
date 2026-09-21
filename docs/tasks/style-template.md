---
title: The style block a new drawing is created with
description: What <style> the app writes into a fresh drawing so a viewer with no theme still shows a marker as a marker — the app's to settle, held to here
author: adammharris
status: done
created: 2026-09-19
updated: 2026-09-20
part_of: '[Tasks](tasks.md)'
---
# The style block a new drawing is created with

**Done 2026-09-20**, in the commit that closes this. The template is
`profile::TEMPLATE` — `tests/fixtures/fresh.svg`, included verbatim — and
`Drawing::fresh()` opens it; the binding has `Drawing.fresh()`, `template()`
and `is_drawing()`, the last being how a host tells a drawing from any other
`.svg`. The fixture test holds it to the profile and to being empty. What it
carries is what `apps/thorn-mac` started with: `<defs>` with the arrow
`<marker>`, and a `<style>` that strokes a box, fills an ink stroke, and
resolves `data-arrow` to the marker. The Diaryx app's `New Drawing` writes
exactly it.

**2026-09-20, later.** The template's ink became `currentColor` off the
root's `color="#222"`, with a `prefers-color-scheme: dark` rule for a
browser; the canvas follows its view's appearance (`CanvasModel.Appearance`)
and draws the picture with `DrawingDocument.ink` set, a `<style>` appended
to what resvg parses and never to the file. resvg skips `@media`, which is
why the editor sets the colour itself rather than relying on the rule.

**2026-09-20, later still.** The `line, path` rule that a bend needs had
reached the arrowhead's own `<path>` inside the `<marker>`, drawing every
head hollow; the template now fills `marker path` on its own. A drawing
made from an earlier template gets these rules the first time it is bent
(`docs/profile.md`, *A connector*), so the template's history is not a
thing a file is stuck with.

The profile does not write a `<style>`; a shape with no fill or stroke
is drawn by SVG's defaults (black fill, no stroke), which is a silhouette
but not a diagram. The app's proposal has a fresh drawing carry a `<style>`
that gives every marker its look with no theme, and this repository should
hold that template to a test once it is settled: a fixture that is exactly
what `New Drawing` writes, and a check that a rendered rectangle has the
stroke and no fill the template says.

Owned by the app's proposal; this is the seam where it lands here.
