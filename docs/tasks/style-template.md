---
title: The style block a new drawing is created with
description: What <style> the app writes into a fresh drawing so a viewer with no theme still shows a marker as a marker — the app's to settle, held to here
author: adammharris
status: open
created: 2026-09-19
updated: 2026-09-19
part_of: '[Tasks](tasks.md)'
---
# The style block a new drawing is created with

The profile does not write a `<style>`; a shape with no fill or stroke
is drawn by SVG's defaults (black fill, no stroke), which is a silhouette
but not a diagram. The app's proposal has a fresh drawing carry a `<style>`
that gives every marker its look with no theme, and this repository should
hold that template to a test once it is settled: a fixture that is exactly
what `New Drawing` writes, and a check that a rendered rectangle has the
stroke and no fill the template says.

Owned by the app's proposal; this is the seam where it lands here.
