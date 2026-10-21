---
title: The workspace's views are not read
part_of: '[Tasks](/docs/tasks/tasks.md)'
status: open
author: adammharris
created: 2026-10-21
description: '`prov check` reports three findings in thorn: the views in `.config/prov.yaml` use retired keys and are not read, so `prov views open-tasks` answers that there is no such view, and every prov command in the workspace warns about them'
updated: 2026-10-21
---

# The workspace's views are not read

## Repro

```sh
prov check          # 3 finding(s), exit 1
prov views open-tasks
# prov: no view named `open-tasks` — this workspace declares none
```

Every other prov command run in the workspace also starts with
`prov: 3 config setting(s) will not take effect (e.g. views.open-tasks)`.

## What is wrong

`open-tasks`, `proposals` and `work` in `.config/prov.yaml` are written with
`group`, `under` and a `where:` mapping, which prov has retired. prov skips
them and says so. `prov check` prints the CEL spelling of each:

```yaml
views:
  open-tasks:
    label: "Open tasks"
    where: "under('[[Tasks]]') && present(status) && !('done' in field('status') || 'dropped' in field('status'))"
    key: "status"
  proposals:
    label: "Proposals"
    where: "under('[[Proposals]]') && present(status)"
    key: "status"
  work:
    label: "All work"
    where: "present(status)"
    key: "status"
```

twig had the same three views and filed the same finding in its
`views-not-read-and-three-bad-links` task, which noted that thorn's belonged
in a thorn task.

## Done when

`prov check` in thorn reports no findings, and `prov views open-tasks` lists
the open tasks.
