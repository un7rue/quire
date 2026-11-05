# Widget designer: your own widget types

> **Status:** ✅ steps 1–3 done 28 Oct 2026 — composite widgets (parts on a grid in one card, shared data and filters, a part edited with its full inspector, types carrying parts), the fine controls, and *Advanced settings…* (JSON checked against the schema); the Quire theme names each built-in widget once. Step 4 (layered chart) later. Earlier: step 1 (widget types) done 24 Oct 2026 — any designer creates them (logged in Activity), a shared library, bankers get changes when published again. A widget studio (open any widget on sample data, edit, save to the library) and the Widgets tab are done too. Next: widgets as theme-independent library items (see below), then steps 2–4.
> **Goal:** a designer builds the widgets they want, down to the small details, and reuses them as types, without
> drawing code and without losing what the app gives every widget (export, fitting, themes, click-a-part, report-user
> permissions, the AI).

## Principle

A custom widget is **assembled from what exists**: a configured widget, or several parts in one card. It is never a
free-form drawing or code (the same rule as "visuals are chart types").

## Steps

1. ✅ **Done 24 Oct 2026 — Widget types.** *Save as widget type…* on any configured widget asks for:
   - a name, an icon and a description;
   - the data it expects (how many fields and values), and sample data for the preview *(not yet: a type keeps the data its widget had; a new widget starts with it and can be rebound in Content)*;
   - which settings report users may still change *(not yet: report users keep the template's usual permissions)*.

   The type appears in the Widgets palette beside the built-in kinds. A widget made from it follows the type: its
   settings sit under the widget's own, as with design views, so changing the type updates every widget using it.
   *Update the type from this widget* and *Detach from the type* are in the widget's menu.
2. **Composite widgets (a widget studio).** A type can be a small grid inside one card holding existing parts:
   figure, mini chart, table rows, text, icon, divider. The data is bound once, with shared filters. Each part keeps
   its full inspector. Examples: fact cards, an account summary block, a KPI with breakdown, a "leader + chart" panel.
   The built-in fact cards can later be rebuilt this way.
3. **The remaining fine controls** (see [designer-controls.md](designer-controls.md) step 4): reference lines and
   bands, category-axis rotation and spacing, legend swatch shape and order, table cell padding. Plus an **Advanced**
   view of a widget's raw settings, checked against the schema.
4. **Later:** a *Layered chart* type (columns + line + target band + points) from a small chart grammar.

Not planned: code widgets (an SVG template plus a script). They would break export fidelity, the fit checks,
themes and report-user safety, and they are a security risk in a bank.

## Widgets apart from themes (agreed direction, 24 Oct 2026)

The designer sees widgets as their own things that themes then style, not as a theme's property. Today the
built-in widgets are each theme's design views, and only your own types live apart from themes. The plan:
- **Widgets library** = built-in widgets plus yours, each defining what it shows and how it's built, and opened in
  the widget studio.
- **Themes** style every widget (colours, fonts, shapes) and may adjust a widget's look for that theme, as a restyle
  does. Widgets use theme colours by reference ($accent, $ink…), so one widget looks right in every theme.
- **Migration:** the built-in design views become built-in library widgets (the same ids in every built-in theme);
  existing templates keep working (a widget's `variant` maps to the library widget, and the theme's view stays its
  per-theme adjustment).
  *Started 24 Oct 2026:* the library lists each built-in widget once; the Quire theme's own ids for 12 shared
  widgets are mapped to the shared ids (`theme_studio::same_widget`). Still to do: give them the same ids in the
  Quire theme itself.

## Decisions to make first

| Question | Options | Suggested |
|---|---|---|
| Who may create and change types | any designer · designers with a "library" right | any designer, with changes logged (Activity) |
| Where types live | inside a theme · in a library every theme can use | the library; each theme styles them through its design views |
| When a type changes | published reports follow at once · only when published again | only when published again (as templates) |

## What it touches

- **Model:** `Widget.type` (the type's id); the type's settings merged under the widget's own (beside the theme's
  design view) in `model::effective`.
- **Library:** `widget_types/` beside the templates library; list, save, update, delete; included when a template
  is published (a snapshot, so reports don't change by themselves).
- **UI:** the Widgets palette group, the save / update / detach menu items, a Design → *Type* row (which type, and
  "set here" marks for what the widget overrides), and for step 2 the widget studio.
- **Checks:** a type used by widgets in two templates; updating it changes both; detaching keeps the look; publishing
  freezes the version; a report user can change only what the type allows.
