/// Report UI tokens, layout, and static progressive-enhancement behavior.
pub(super) fn render_style(html: &mut String) {
    html.push_str(
        r#"<style>
:root {
  color-scheme: dark;
  --bg: #090909;
  --surface: #121212;
  --surface-raised: #181818;
  --line: #2d2d2f;
  --line-strong: #3a3a3d;
  --text: #f4f4f5;
  --muted: #ababaf;
  --accent: #8fdcff;
  --focus: #c7f0ff;
  --error: #ff6b6b;
  --warn: #ffd166;
  --pass: #64d98a;
}
* { box-sizing: border-box; }
[hidden] { display: none !important; }
body {
  margin: 0;
  background: var(--bg);
  color: var(--text);
  font: 15px/1.5 ui-sans-serif, system-ui, -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif;
}
main { max-width: 1120px; margin: 0 auto; padding: 32px 24px 52px; }
h1, h2, h3, p { margin: 0; }
h1 { font-size: 2rem; letter-spacing: 0; line-height: 1.15; }
h2 { margin-top: 30px; font-size: 1.25rem; }
.topline { color: var(--muted); margin-bottom: 8px; text-transform: uppercase; letter-spacing: .08em; font-size: 12px; }
.hero { border-bottom: 1px solid var(--line); padding-bottom: 22px; }
.verdict { display: inline-flex; align-items: center; gap: 8px; margin-top: 14px; padding: 6px 9px; border: 1px solid var(--line); border-radius: 6px; }
.pass { color: var(--pass); }
.fail, .error { color: var(--error); }
.warning { color: var(--warn); }
.metrics, .groups { display: grid; grid-template-columns: repeat(auto-fit, minmax(160px, 1fr)); gap: 12px 18px; margin-top: 22px; }
.metric { border-top: 1px solid var(--line); padding-top: 10px; }
.group, .step { background: var(--surface); border: 1px solid var(--line); border-radius: 8px; padding: 14px; }
.metric span, .group span, .label { color: var(--muted); display: block; font-size: 12px; text-transform: uppercase; letter-spacing: .06em; }
.metric strong { display: block; margin-top: 4px; font-size: 1.35rem; }
.group p { margin-top: 8px; color: var(--text); }
.finding-tools { display: grid; gap: 12px; margin-top: 14px; padding: 14px; background: var(--surface); border: 1px solid var(--line); border-radius: 8px; }
.finding-tools label, .filter-label { display: grid; gap: 6px; color: var(--muted); font-size: 12px; text-transform: uppercase; letter-spacing: .06em; }
.finding-tools input { width: 100%; min-height: 44px; border: 1px solid var(--line-strong); border-radius: 6px; background: var(--bg); color: var(--text); font: inherit; padding: 8px 10px; }
.finding-tools input:focus { outline: 2px solid var(--focus); outline-offset: 2px; }
.filter-set { display: grid; gap: 8px; }
.filter-buttons { display: flex; flex-wrap: wrap; gap: 8px; }
.type-filter { min-height: 44px; border: 1px solid var(--line-strong); border-radius: 6px; background: var(--bg); color: var(--text); cursor: pointer; font: inherit; padding: 8px 10px; text-align: left; }
.type-filter:hover { border-color: var(--muted); }
.type-filter:focus-visible { outline: 2px solid var(--focus); outline-offset: 2px; }
.type-filter[aria-pressed="true"] { border-color: var(--accent); color: var(--accent); background: #10232b; }
.finding-status, .empty-results { grid-column: 1 / -1; color: var(--muted); font-size: 13px; }
.finding-groups { display: grid; gap: 12px; margin-top: 14px; }
.finding-group { background: var(--surface); border: 1px solid var(--line); border-radius: 8px; overflow: hidden; }
.finding-group > summary { display: flex; align-items: center; justify-content: space-between; gap: 16px; cursor: pointer; list-style: none; padding: 14px 16px; }
.finding-group > summary::-webkit-details-marker { display: none; }
.finding-group > summary:hover { background: var(--surface-raised); }
.finding-group > summary:focus-visible { outline: 2px solid var(--focus); outline-offset: -2px; }
.finding-group[open] > summary { border-bottom: 1px solid var(--line); }
.summary-title { display: grid; grid-template-columns: auto minmax(0, 1fr); gap: 10px; align-items: start; }
.summary-text { display: grid; gap: 3px; min-width: 0; }
.group-number { color: var(--muted); font-variant-numeric: tabular-nums; }
.summary-title strong { font-size: 1rem; }
.summary-right { display: inline-flex; align-items: center; gap: 12px; }
.summary-meta { color: var(--muted); font-size: 13px; text-align: right; }
.chevron { width: 9px; height: 9px; border-right: 2px solid var(--muted); border-bottom: 2px solid var(--muted); transform: rotate(45deg); transform-origin: center; }
.finding-group[open] .chevron { transform: rotate(225deg); }
.findings { display: grid; gap: 12px; margin-top: 14px; }
.finding-group .findings { margin-top: 0; padding: 12px; }
.finding { background: var(--surface-raised); border: 1px solid var(--line); border-radius: 8px; padding: 14px; }
.finding header { display: flex; flex-wrap: wrap; align-items: baseline; justify-content: space-between; gap: 10px; margin-bottom: 12px; }
.finding h3 { font-size: 1rem; }
.rule, .location, .mono { font-family: ui-monospace, SFMono-Regular, Menlo, Consolas, monospace; }
.rule { color: var(--muted); font-size: 13px; overflow-wrap: anywhere; }
.section { margin-top: 10px; }
.section p { margin-top: 3px; overflow-wrap: anywhere; }
.evidence { margin: 8px 0 0; padding-left: 18px; color: var(--text); }
.evidence li { margin: 3px 0; overflow-wrap: anywhere; }
.command { overflow-wrap: anywhere; color: var(--accent); }
a { color: var(--accent); text-decoration: none; }
a:hover { text-decoration: underline; }
@media (max-width: 720px) {
  main { padding: 24px 16px 44px; }
  h1 { font-size: 1.65rem; }
  .finding-group > summary { align-items: flex-start; flex-direction: column; }
  .summary-right { justify-content: space-between; width: 100%; }
  .summary-meta { text-align: left; }
}
@media (prefers-reduced-motion: no-preference) {
  .finding-group > summary, .finding-tools input, .type-filter { transition: background-color .16s ease, border-color .16s ease, color .16s ease; }
  .chevron { transition: transform .16s ease; }
}
</style>
"#,
    );
}

pub(super) fn render_interaction_script(html: &mut String) {
    html.push_str(
        r#"<script>
(() => {
  const controls = document.querySelector("[data-finding-controls]");
  if (!controls) return;
  const searchInput = controls.querySelector("[data-finding-search]");
  const filterButtons = Array.from(controls.querySelectorAll("[data-finding-filter]"));
  const status = controls.querySelector("[data-finding-status]");
  const emptyResults = document.querySelector("[data-empty-results]");
  const groups = Array.from(document.querySelectorAll("[data-finding-group]"));
  if (!searchInput || filterButtons.length === 0 || !status || groups.length === 0) return;
  const totalFindings = Number(status.dataset.total || 0);
  let selectedType = "";

  const syncButtons = () => {
    for (const button of filterButtons) {
      button.setAttribute("aria-pressed", button.dataset.kind === selectedType ? "true" : "false");
    }
  };

  const update = () => {
    const query = searchInput.value.trim().toLowerCase();
    let visibleFindings = 0;
    let visibleGroups = 0;

    for (const group of groups) {
      const typeMatches = !selectedType || group.dataset.kind === selectedType;
      let groupMatches = 0;
      const findings = Array.from(group.querySelectorAll("[data-finding]"));

      for (const finding of findings) {
        const textMatches = !query || finding.textContent.toLowerCase().includes(query);
        const visible = typeMatches && textMatches;
        finding.hidden = !visible;
        if (visible) groupMatches += 1;
      }

      group.hidden = groupMatches === 0;
      if (groupMatches > 0) {
        visibleGroups += 1;
        visibleFindings += groupMatches;
      }
    }

    status.textContent = `Showing ${visibleFindings} of ${totalFindings} findings in ${visibleGroups} ${visibleGroups === 1 ? "type" : "types"}.`;
    if (emptyResults) emptyResults.hidden = visibleFindings !== 0;
  };

  searchInput.addEventListener("input", update);
  for (const button of filterButtons) {
    button.addEventListener("click", () => {
      selectedType = button.dataset.kind || "";
      syncButtons();
      if (selectedType) {
        for (const group of groups) group.open = group.dataset.kind === selectedType;
      }
      update();
    });
  }
})();
</script>
"#,
    );
}
