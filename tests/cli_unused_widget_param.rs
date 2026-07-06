use std::fs;

use dart_decimate::cli::run_from;
use serde_json::{Value, json};
use tempfile::TempDir;

#[test]
fn check_reports_unused_widget_field_formal() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = widget_fixture()?;
    let mut output = Vec::new();

    let code = run_from(
        [
            "dart-decimate",
            "check",
            fixture.path().to_str().unwrap_or("."),
            "--format",
            "json",
            "--entry",
            "lib/main.dart",
        ],
        &mut output,
    )?;

    let json = serde_json::from_slice::<Value>(&output)?;
    assert_eq!(code, 0);
    assert_eq!(json["verdict"], "pass");
    assert_eq!(json["summary"]["unused_widget_params"], 2);

    let finding = unused_widget_param_finding(&json);
    assert_eq!(finding["kind"], "unused-widget-param");
    assert_eq!(finding["severity"], "warning");
    assert_eq!(finding["path"], "lib/widgets.dart");
    assert_eq!(finding["line"], 3);
    assert_eq!(finding["safe_to_delete"], false);
    assert_eq!(finding["files"], json!([]));
    assert_eq!(finding["edge"], Value::Null);
    assert_eq!(finding["actions"][0]["action"], "review-widget-param");
    assert_eq!(finding["actions"][0]["auto_fixable"], false);
    assert_eq!(
        finding["actions"][0]["target_symbol"],
        "UnusedFieldFormal.unused"
    );
    assert_eq!(
        finding["actions"][0]["suppression_comment"],
        "// dart-decimate-ignore-next-line unused-widget-param"
    );
    assert_no_widget_param_for(&json, "title");
    assert_no_widget_param_for(&json, "count");
    assert_no_widget_param_for(&json, "label");
    assert_no_widget_param_for(&json, "key");
    assert_widget_param_for(&json, "UnusedExplicit.unused");
    assert_no_widget_param_for(&json, "usedExplicit");

    Ok(())
}

#[test]
fn check_ignores_generated_widget_files() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(&fixture, "pubspec.yaml", "name: app\n")?;
    write(
        &fixture,
        "lib/main.dart",
        "import 'generated_widget.g.dart';\nvoid main() { GeneratedWidget(unused: 'x'); }\n",
    )?;
    write(
        &fixture,
        "lib/generated_widget.g.dart",
        r"
class GeneratedWidget extends StatelessWidget {
  const GeneratedWidget({super.key, required this.unused});
  final String unused;
  Widget build(BuildContext context) => const SizedBox();
}
",
    )?;
    let mut output = Vec::new();

    let code = run_from(
        [
            "dart-decimate",
            "check",
            fixture.path().to_str().unwrap_or("."),
            "--format",
            "json",
            "--entry",
            "lib/main.dart",
        ],
        &mut output,
    )?;

    let json = serde_json::from_slice::<Value>(&output)?;
    assert_eq!(code, 0);
    assert_eq!(json["summary"]["unused_widget_params"], 0);
    assert!(json["findings"].as_array().is_some_and(Vec::is_empty));

    Ok(())
}

#[test]
fn unused_widget_param_rule_can_error_or_turn_off() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = widget_fixture()?;
    write(
        &fixture,
        ".dart-decimaterc.json",
        r#"{ "rules": { "unused-component-prop": "error" } }"#,
    )?;
    let mut output = Vec::new();

    let code = run_from(
        [
            "dart-decimate",
            "check",
            fixture.path().to_str().unwrap_or("."),
            "--format",
            "json",
            "--entry",
            "lib/main.dart",
        ],
        &mut output,
    )?;

    let json = serde_json::from_slice::<Value>(&output)?;
    assert_eq!(code, 1);
    assert_eq!(json["verdict"], "fail");
    assert_eq!(unused_widget_param_finding(&json)["severity"], "error");

    write(
        &fixture,
        ".dart-decimaterc.json",
        r#"{ "rules": { "unused-widget-param": "off" } }"#,
    )?;
    output.clear();

    let code = run_from(
        [
            "dart-decimate",
            "check",
            fixture.path().to_str().unwrap_or("."),
            "--format",
            "json",
            "--entry",
            "lib/main.dart",
        ],
        &mut output,
    )?;

    let json = serde_json::from_slice::<Value>(&output)?;
    assert_eq!(code, 0);
    assert_eq!(json["verdict"], "pass");
    assert_eq!(json["summary"]["unused_widget_params"], 0);
    assert!(json["findings"].as_array().is_some_and(Vec::is_empty));

    Ok(())
}

#[test]
fn check_counts_pattern_reads_and_view_data_forwarding_as_usage()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(&fixture, "pubspec.yaml", "name: app\n")?;
    write(
        &fixture,
        "lib/main.dart",
        r"
import 'widgets.dart';

void main() {
  PatternDisplay(value: 'ok');
  ForwardingPanel(items: const ['a'], height: 240, suffix: 'kg', unused: 'x');
}
",
    )?;
    write(
        &fixture,
        "lib/widgets.dart",
        r"
class PatternDisplay extends StatelessWidget {
  const PatternDisplay({super.key, required this.value});
  final String value;
  Widget build(BuildContext context) {
    final PatternDisplay(:value) = this;
    return Text(value);
  }
}

class ForwardingPanel extends StatefulWidget {
  const ForwardingPanel({
    super.key,
    required this.items,
    required this.height,
    required this.suffix,
    required this.unused,
  });
  final List<String> items;
  final double height;
  final String suffix;
  final String unused;
  State<ForwardingPanel> createState() => _ForwardingPanelState();
}

class _ForwardingPanelState extends State<ForwardingPanel> {
  Widget build(BuildContext context) {
    return ForwardedBody(
      viewData: ForwardedViewData.fromOwner(
        selectedIndex: 0,
        source: widget,
      ),
    );
  }
}

class ForwardedViewData {
  ForwardedViewData.fromOwner({
    required this.selectedIndex,
    required ForwardingPanel source,
  })  : items = source.items,
        height = source.height,
        suffix = source.suffix;
  final int selectedIndex;
  final List<String> items;
  final double height;
  final String suffix;
}

class ForwardedBody extends StatelessWidget {
  const ForwardedBody({super.key, required this.viewData});
  final ForwardedViewData viewData;
  Widget build(BuildContext context) => Text('${viewData.items.length}${viewData.suffix}');
}
",
    )?;
    let mut output = Vec::new();

    let code = run_from(
        [
            "dart-decimate",
            "check",
            fixture.path().to_str().unwrap_or("."),
            "--format",
            "json",
            "--entry",
            "lib/main.dart",
        ],
        &mut output,
    )?;

    let json = serde_json::from_slice::<Value>(&output)?;
    assert_eq!(code, 0);
    assert_eq!(json["verdict"], "pass");
    assert_no_widget_target(&json, "PatternDisplay.value");
    assert_no_widget_target(&json, "ForwardingPanel.items");
    assert_no_widget_target(&json, "ForwardingPanel.height");
    assert_no_widget_target(&json, "ForwardingPanel.suffix");
    assert_widget_param_for(&json, "ForwardingPanel.unused");

    Ok(())
}

#[test]
fn check_forwarded_usage_matches_the_read_constructor_parameter()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(&fixture, "pubspec.yaml", "name: app\n")?;
    write(
        &fixture,
        "lib/main.dart",
        r"
import 'widgets.dart';

void main() {
  ForwardingPanel(items: const ['a'], unused: 'x');
}
",
    )?;
    write(
        &fixture,
        "lib/widgets.dart",
        r"
class ForwardingPanel extends StatefulWidget {
  const ForwardingPanel({super.key, required this.items, required this.unused});
  final List<String> items;
  final String unused;
  State<ForwardingPanel> createState() => _ForwardingPanelState();
}

class _ForwardingPanelState extends State<ForwardingPanel> {
  Widget build(BuildContext context) {
    final data = ForwardedViewData.fromOwner(
      source: widget,
      other: const ForwardingPanel(items: ['fallback'], unused: 'fallback'),
    );
    return Text('${data.items.length}');
  }
}

class ForwardedViewData {
  ForwardedViewData.fromOwner({
    required ForwardingPanel source,
    required ForwardingPanel other,
  }) : items = other.items;
  final List<String> items;
}
",
    )?;
    let mut output = Vec::new();

    let code = run_from(
        [
            "dart-decimate",
            "check",
            fixture.path().to_str().unwrap_or("."),
            "--format",
            "json",
            "--entry",
            "lib/main.dart",
        ],
        &mut output,
    )?;

    let json = serde_json::from_slice::<Value>(&output)?;
    assert_eq!(code, 0);
    assert_eq!(json["verdict"], "pass");
    assert_widget_param_for(&json, "ForwardingPanel.items");
    assert_widget_param_for(&json, "ForwardingPanel.unused");

    Ok(())
}

#[test]
fn check_counts_stateless_widget_forwarding_as_usage() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(&fixture, "pubspec.yaml", "name: app\n")?;
    write(
        &fixture,
        "lib/main.dart",
        r"
import 'widgets.dart';

void main() {
  StatelessForwardingPanel(title: 'ready', unused: 'x');
}
",
    )?;
    write(
        &fixture,
        "lib/widgets.dart",
        r"
class StatelessForwardingPanel extends StatelessWidget {
  const StatelessForwardingPanel({super.key, required this.title, required this.unused});
  final String title;
  final String unused;
  Widget build(BuildContext context) {
    return ForwardedTitleBody(
      viewData: ForwardedTitleData.fromOwner(source: this),
    );
  }
}

class ForwardedTitleData {
  ForwardedTitleData.fromOwner({required StatelessForwardingPanel source})
      : title = source.title;
  final String title;
}

class ForwardedTitleBody extends StatelessWidget {
  const ForwardedTitleBody({super.key, required this.viewData});
  final ForwardedTitleData viewData;
  Widget build(BuildContext context) => Text(viewData.title);
}
",
    )?;
    let mut output = Vec::new();

    let code = run_from(
        [
            "dart-decimate",
            "check",
            fixture.path().to_str().unwrap_or("."),
            "--format",
            "json",
            "--entry",
            "lib/main.dart",
        ],
        &mut output,
    )?;

    let json = serde_json::from_slice::<Value>(&output)?;
    assert_eq!(code, 0);
    assert_eq!(json["verdict"], "pass");
    assert_no_widget_target(&json, "StatelessForwardingPanel.title");
    assert_widget_param_for(&json, "StatelessForwardingPanel.unused");

    Ok(())
}

#[test]
fn check_counts_only_matching_widget_object_pattern_fields_as_usage()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(&fixture, "pubspec.yaml", "name: app\n")?;
    write(
        &fixture,
        "lib/main.dart",
        r"
import 'widgets.dart';

void main() {
  DirectPatternWidget(used: 'ok', unused: 'x');
  StaticPatternWidget(title: 'T', subtitle: 'S', unused: 'x');
  ChartDisplay(data: const [1], selectedIndex: 0, tooltipSuffix: 'kg', unused: 'x');
  IfCasePatternWidget(used: 'ok', unused: 'x');
  SwitchPatternWidget(used: 'ok', unused: 'x');
  OtherPatternWidget(used: 'ok', unused: 'x');
  DifferentFieldPatternWidget(used: 'ok', unused: 'x');
  LocalShadowWidget(title: 'real');
  HelperCollisionWidget(used: 'real', unused: 'x');
  AliasScopeWidget(used: 'real', unused: 'x');
  UnrelatedPatternWidget(value: 'real');
  DeadHelperPatternWidget(used: 'real', unused: 'x');
  OtherInstancePatternWidget(used: 'real', unused: 'x');
  LocalHelperLeakWidget(used: 'real', unused: 'x');
  ThisMethodPatternWidget(used: 'real', unused: 'x');
  WrappedPatternWidget(title: 'real', subtitle: 'sub', unused: 'x');
  NestedHelperShadowWidget(used: 'real', unused: 'x');
  LocalTopLevelHelperShadowWidget(used: 'real', unused: 'x');
  StateOldWidgetPatternWidget(used: 'real', unused: 'x');
  StateRootShadowWidget(used: 'real', unused: 'x');
  StateOwnerParamShadowWidget(used: 'real', unused: 'x');
  StatePatternRootShadowWidget(used: 'real', unused: 'x');
  ParameterShadowHelperWidget(used: 'real', unused: 'x');
  MemberShadowHelperWidget(used: 'real', unused: 'x');
}
",
    )?;
    write(&fixture, "lib/widgets.dart", OBJECT_PATTERN_WIDGETS_SOURCE)?;
    let mut output = Vec::new();

    let code = run_from(
        [
            "dart-decimate",
            "check",
            fixture.path().to_str().unwrap_or("."),
            "--format",
            "json",
            "--entry",
            "lib/main.dart",
        ],
        &mut output,
    )?;

    let json = serde_json::from_slice::<Value>(&output)?;
    assert_eq!(code, 0);
    assert_eq!(json["verdict"], "pass");
    let targets = unused_widget_targets(&json);

    assert_targets_contain(&targets, OBJECT_PATTERN_UNUSED_TARGETS);
    assert_targets_do_not_contain(&targets, OBJECT_PATTERN_USED_TARGETS);

    Ok(())
}

const OBJECT_PATTERN_UNUSED_TARGETS: &[&str] = &[
    "DirectPatternWidget.unused",
    "StaticPatternWidget.unused",
    "ChartDisplay.unused",
    "IfCasePatternWidget.unused",
    "SwitchPatternWidget.unused",
    "OtherPatternWidget.used",
    "OtherPatternWidget.unused",
    "DifferentFieldPatternWidget.unused",
    "LocalShadowWidget.title",
    "HelperCollisionWidget.used",
    "HelperCollisionWidget.unused",
    "AliasScopeWidget.used",
    "AliasScopeWidget.unused",
    "UnrelatedPatternWidget.value",
    "DeadHelperPatternWidget.used",
    "DeadHelperPatternWidget.unused",
    "OtherInstancePatternWidget.used",
    "OtherInstancePatternWidget.unused",
    "LocalHelperLeakWidget.used",
    "LocalHelperLeakWidget.unused",
    "ThisMethodPatternWidget.unused",
    "WrappedPatternWidget.unused",
    "NestedHelperShadowWidget.used",
    "NestedHelperShadowWidget.unused",
    "LocalTopLevelHelperShadowWidget.used",
    "LocalTopLevelHelperShadowWidget.unused",
    "StateOldWidgetPatternWidget.unused",
    "StateRootShadowWidget.used",
    "StateRootShadowWidget.unused",
    "StateOwnerParamShadowWidget.used",
    "StateOwnerParamShadowWidget.unused",
    "StatePatternRootShadowWidget.used",
    "StatePatternRootShadowWidget.unused",
    "ParameterShadowHelperWidget.used",
    "ParameterShadowHelperWidget.unused",
    "MemberShadowHelperWidget.used",
    "MemberShadowHelperWidget.unused",
];

const OBJECT_PATTERN_USED_TARGETS: &[&str] = &[
    "DirectPatternWidget.used",
    "StaticPatternWidget.title",
    "StaticPatternWidget.subtitle",
    "ChartDisplay.data",
    "ChartDisplay.selectedIndex",
    "ChartDisplay.tooltipSuffix",
    "IfCasePatternWidget.used",
    "SwitchPatternWidget.used",
    "DifferentFieldPatternWidget.used",
    "ThisMethodPatternWidget.used",
    "WrappedPatternWidget.title",
    "WrappedPatternWidget.subtitle",
    "StateOldWidgetPatternWidget.used",
];

const OBJECT_PATTERN_WIDGETS_SOURCE: &str = r"
class DirectPatternWidget extends StatelessWidget {
  const DirectPatternWidget({super.key, required this.used, required this.unused});
  final String used;
  final String unused;
  Widget build(BuildContext context) {
    final DirectPatternWidget(:used) = this;
    return Text(used);
  }
}

class StaticPatternWidget extends StatelessWidget {
  const StaticPatternWidget({
    super.key,
    required this.title,
    required this.subtitle,
    required this.unused,
  });
  final String title;
  final String subtitle;
  final String unused;
  static String label(StaticPatternWidget widget) {
    final StaticPatternWidget(:title, :subtitle) = widget;
    return '$title - $subtitle';
  }
  Widget build(BuildContext context) => Text(label(this));
}

class ChartDisplay extends StatelessWidget {
  const ChartDisplay({
    super.key,
    required this.data,
    required this.selectedIndex,
    required this.tooltipSuffix,
    required this.unused,
  });
  final List<double> data;
  final int selectedIndex;
  final String tooltipSuffix;
  final String unused;
  Widget build(BuildContext context) {
    final details = ChartDetails.fromDisplay(this);
    return Text(details);
  }
}

abstract final class ChartDetails {
  static String fromDisplay(ChartDisplay display) {
    final ChartDisplay(:data, :selectedIndex, tooltipSuffix: suffix) = display;
    return '${data[selectedIndex]}$suffix';
  }
}

class IfCasePatternWidget extends StatelessWidget {
  const IfCasePatternWidget({super.key, required this.used, required this.unused});
  final String used;
  final String unused;
  Widget build(BuildContext context) {
    final Object value = this;
    if (value case IfCasePatternWidget(:used)) {
      return Text(used);
    }
    return const SizedBox.shrink();
  }
}

class SwitchPatternWidget extends StatelessWidget {
  const SwitchPatternWidget({super.key, required this.used, required this.unused});
  final String used;
  final String unused;
  Widget build(BuildContext context) {
    return switch (this) {
      SwitchPatternWidget(:used) => Text(used),
      _ => const SizedBox.shrink(),
    };
  }
}

class OtherWidget {
  const OtherWidget({required this.used});
  final String used;
}

class OtherPatternWidget extends StatelessWidget {
  const OtherPatternWidget({super.key, required this.used, required this.unused});
  final String used;
  final String unused;
  Widget build(BuildContext context) {
    final other = const OtherWidget(used: 'x');
    final OtherWidget(:used) = other;
    return Text(used);
  }
}

class DifferentFieldPatternWidget extends StatelessWidget {
  const DifferentFieldPatternWidget({super.key, required this.used, required this.unused});
  final String used;
  final String unused;
  Widget build(BuildContext context) {
    final DifferentFieldPatternWidget(:used) = this;
    return Text(used);
  }
}

class LocalShadowWidget extends StatelessWidget {
  const LocalShadowWidget({super.key, required this.title});
  final String title;
  Widget build(BuildContext context) {
    const title = 'local';
    return Text(title);
  }
}

class HelperCollisionWidget extends StatelessWidget {
  const HelperCollisionWidget({super.key, required this.used, required this.unused});
  final String used;
  final String unused;
  Widget build(BuildContext context) => Text(fromDisplay(this));
}

String fromDisplay(HelperCollisionWidget widget) => 'not a pattern helper';

abstract final class HelperCollisionDetails {
  static String fromDisplay(HelperCollisionWidget widget) {
    final HelperCollisionWidget(:used) = widget;
    return used;
  }
}

class AliasScopeWidget extends StatelessWidget {
  const AliasScopeWidget({super.key, required this.used, required this.unused});
  final String used;
  final String unused;
  Widget build(BuildContext context) {
    final candidate = AliasScopeWidget(used: 'other', unused: 'other');
    final AliasScopeWidget(:used) = candidate;
    {
      final candidate = this;
      Text(candidate.used);
    }
    return Text(used);
  }
}

class UnrelatedPatternWidget extends StatelessWidget {
  const UnrelatedPatternWidget({super.key, required this.value});
  final String value;
  Widget build(BuildContext context) => const SizedBox.shrink();
}

class DeadHelperPatternWidget extends StatelessWidget {
  const DeadHelperPatternWidget({super.key, required this.used, required this.unused});
  final String used;
  final String unused;
  Widget build(BuildContext context) => const SizedBox.shrink();
}

String deadHelperPattern(DeadHelperPatternWidget widget) {
  final DeadHelperPatternWidget(:used) = widget;
  return used;
}

class OtherInstancePatternWidget extends StatelessWidget {
  const OtherInstancePatternWidget({super.key, required this.used, required this.unused});
  final String used;
  final String unused;
  Widget build(BuildContext context) {
    const other = OtherInstancePatternWidget(used: 'other', unused: 'other');
    final OtherInstancePatternWidget(:used) = other;
    return Text(used);
  }
}

class LocalHelperLeakWidget extends StatelessWidget {
  const LocalHelperLeakWidget({super.key, required this.used, required this.unused});
  final String used;
  final String unused;
  Widget build(BuildContext context) => Text(helper(this));
}

void ownerOfDeadLocalHelper() {
  String helper(LocalHelperLeakWidget widget) {
    final LocalHelperLeakWidget(:used) = widget;
    return used;
  }
  helper(const LocalHelperLeakWidget(used: 'dead', unused: 'dead'));
}

class ThisMethodPatternWidget extends StatelessWidget {
  const ThisMethodPatternWidget({super.key, required this.used, required this.unused});
  final String used;
  final String unused;
  String fromDisplay(ThisMethodPatternWidget widget) {
    final ThisMethodPatternWidget(:used) = widget;
    return used;
  }
  Widget build(BuildContext context) => Text(this.fromDisplay(this));
}

class WrappedPatternWidget extends StatelessWidget {
  const WrappedPatternWidget({
    super.key,
    required this.title,
    required this.subtitle,
    required this.unused,
  });
  final String? title;
  final Object subtitle;
  final String unused;
  Widget build(BuildContext context) {
    final WrappedPatternWidget(:title?, :subtitle as String) = this;
    return Text('$title$subtitle');
  }
}

class NestedHelperShadowWidget extends StatelessWidget {
  const NestedHelperShadowWidget({super.key, required this.used, required this.unused});
  final String used;
  final String unused;
  Widget build(BuildContext context) => Text(nestedHelperShadow(this));
}

String nestedHelperShadow(NestedHelperShadowWidget widget) {
  String read(NestedHelperShadowWidget widget) {
    final NestedHelperShadowWidget(:used) = widget;
    return used;
  }
  return read(const NestedHelperShadowWidget(used: 'other', unused: 'other'));
}

class LocalTopLevelHelperShadowWidget extends StatelessWidget {
  const LocalTopLevelHelperShadowWidget({super.key, required this.used, required this.unused});
  final String used;
  final String unused;
  Widget build(BuildContext context) {
    String topLevelShadowedHelper(LocalTopLevelHelperShadowWidget widget) => 'local';
    return Text(topLevelShadowedHelper(this));
  }
}

String topLevelShadowedHelper(LocalTopLevelHelperShadowWidget widget) {
  final LocalTopLevelHelperShadowWidget(:used) = widget;
  return used;
}

class StateOldWidgetPatternWidget extends StatefulWidget {
  const StateOldWidgetPatternWidget({super.key, required this.used, required this.unused});
  final String used;
  final String unused;
  State<StateOldWidgetPatternWidget> createState() => _StateOldWidgetPatternWidgetState();
}

class _StateOldWidgetPatternWidgetState extends State<StateOldWidgetPatternWidget> {
  void didUpdateWidget(StateOldWidgetPatternWidget oldWidget) {
    final StateOldWidgetPatternWidget(:used) = oldWidget;
    Text(used);
  }
  Widget build(BuildContext context) => const SizedBox.shrink();
}

class StateRootShadowWidget extends StatefulWidget {
  const StateRootShadowWidget({super.key, required this.used, required this.unused});
  final String used;
  final String unused;
  State<StateRootShadowWidget> createState() => _StateRootShadowWidgetState();
}

class _StateRootShadowWidgetState extends State<StateRootShadowWidget> {
  Widget build(BuildContext context) {
    final widget = StateRootShadowWidget(used: 'other', unused: 'other');
    final StateRootShadowWidget(:used) = widget;
    return Text(used);
  }
}

class StateOwnerParamShadowWidget extends StatefulWidget {
  const StateOwnerParamShadowWidget({super.key, required this.used, required this.unused});
  final String used;
  final String unused;
  State<StateOwnerParamShadowWidget> createState() => _StateOwnerParamShadowWidgetState();
}

class _StateOwnerParamShadowWidgetState extends State<StateOwnerParamShadowWidget> {
  void show(StateOwnerParamShadowWidget widget) {
    final StateOwnerParamShadowWidget(:used) = widget;
    Text(used);
  }
  Widget build(BuildContext context) => const SizedBox.shrink();
}

class StatePatternRootShadowWidget extends StatefulWidget {
  const StatePatternRootShadowWidget({super.key, required this.used, required this.unused});
  final String used;
  final String unused;
  State<StatePatternRootShadowWidget> createState() => _StatePatternRootShadowWidgetState();
}

class _StatePatternRootShadowWidgetState extends State<StatePatternRootShadowWidget> {
  Widget build(BuildContext context) {
    final (widget,) = (StatePatternRootShadowWidget(used: 'other', unused: 'other'),);
    final StatePatternRootShadowWidget(:used) = widget;
    return Text(used);
  }
}

class ParameterShadowHelperWidget extends StatelessWidget {
  const ParameterShadowHelperWidget({super.key, required this.used, required this.unused});
  final String used;
  final String unused;
  Widget build(
    BuildContext context,
    String Function(ParameterShadowHelperWidget) parameterShadowHelper,
  ) {
    return Text(parameterShadowHelper(this));
  }
}

String parameterShadowHelper(ParameterShadowHelperWidget widget) {
  final ParameterShadowHelperWidget(:used) = widget;
  return used;
}

class MemberShadowHelperWidget extends StatelessWidget {
  const MemberShadowHelperWidget({super.key, required this.used, required this.unused});
  final String used;
  final String unused;
  String memberShadowHelper(MemberShadowHelperWidget widget) => 'member';
  Widget build(BuildContext context) => Text(memberShadowHelper(this));
}

String memberShadowHelper(MemberShadowHelperWidget widget) {
  final MemberShadowHelperWidget(:used) = widget;
  return used;
}

class OtherThing {
  const OtherThing(this.value);
  final String value;
}

String helper(OtherThing thing) {
  final OtherThing(:value) = thing;
  return value;
}
";

fn widget_fixture() -> Result<TempDir, std::io::Error> {
    let fixture = tempfile::tempdir()?;
    write(&fixture, "pubspec.yaml", "name: app\n")?;
    write(
        &fixture,
        "lib/main.dart",
        r"
import 'widgets.dart';

void main() {
  UsedInBuild(title: 'ok');
  UsedViaState(count: 1);
  Parent(label: 'child');
  UnusedFieldFormal(unused: 'x', used: 'y');
  UnusedExplicit(unused: 'x', usedExplicit: 'y');
}
",
    )?;
    write(
        &fixture,
        "lib/widgets.dart",
        r"
class UnusedFieldFormal extends StatelessWidget {
  const UnusedFieldFormal({super.key, required this.unused, required this.used});
  final String unused;
  final String used;
  Widget build(BuildContext context) => Text(used);
}

class UsedInBuild extends StatelessWidget {
  const UsedInBuild({super.key, required this.title});
  final String title;
  Widget build(BuildContext context) => Text('hello $title');
}

class UsedViaState extends StatefulWidget {
  const UsedViaState({super.key, required this.count});
  final int count;
  State<UsedViaState> createState() => _UsedViaStateState();
}

class _UsedViaStateState extends State<UsedViaState> {
  Widget build(BuildContext context) => Text('${widget.count}');
}

class Parent extends StatelessWidget {
  const Parent({super.key, required this.label});
  final String label;
  Widget build(BuildContext context) => Child(label: label);
}

class Child extends StatelessWidget {
  const Child({super.key, required this.label});
  final String label;
  Widget build(BuildContext context) => Text(label);
}

class UnusedExplicit extends StatelessWidget {
  const UnusedExplicit({
    super.key,
    required String unused,
    required String usedExplicit,
  })  : unused = unused,
        usedExplicit = usedExplicit;
  final String unused;
  final String usedExplicit;
  Widget build(BuildContext context) => Text(usedExplicit);
}
",
    )?;
    Ok(fixture)
}

#[test]
fn check_respects_widget_field_shadowing_scope_boundaries() -> Result<(), Box<dyn std::error::Error>>
{
    let fixture = tempfile::tempdir()?;
    write(&fixture, "pubspec.yaml", "name: app\n")?;
    write(
        &fixture,
        "lib/main.dart",
        r"
import 'widgets.dart';

void main() {
  CatchBetweenWidget(title: 'real');
  LocalFunctionShadowWidget(title: 'real');
  FunctionTypedParamWidget(title: 'real');
}
",
    )?;
    write(
        &fixture,
        "lib/widgets.dart",
        r"
class CatchBetweenWidget extends StatelessWidget {
  const CatchBetweenWidget({super.key, required this.title});
  final String title;
  Widget build(BuildContext context) {
    try {
      throw StateError('x');
    } on FormatException catch (title) {
      return Text(title.message);
    } on StateError {
      return Text(title);
    }
  }
}

class LocalFunctionShadowWidget extends StatelessWidget {
  const LocalFunctionShadowWidget({super.key, required this.title});
  final String title;
  Widget build(BuildContext context) {
    String title() => 'local';
    return Text(title());
  }
}

class FunctionTypedParamWidget extends StatelessWidget {
  const FunctionTypedParamWidget({super.key, required this.title});
  final String title;
  Widget build(BuildContext context, String Function(String title) format) => Text(title);
}
",
    )?;
    let mut output = Vec::new();

    let code = run_from(
        [
            "dart-decimate",
            "check",
            fixture.path().to_str().unwrap_or("."),
            "--format",
            "json",
            "--entry",
            "lib/main.dart",
        ],
        &mut output,
    )?;

    let json = serde_json::from_slice::<Value>(&output)?;
    assert_eq!(code, 0);
    assert_eq!(json["verdict"], "pass");
    assert_no_widget_target(&json, "CatchBetweenWidget.title");
    assert_widget_param_for(&json, "LocalFunctionShadowWidget.title");
    assert_no_widget_target(&json, "FunctionTypedParamWidget.title");

    Ok(())
}

fn unused_widget_param_finding(json: &Value) -> &Value {
    let Some(finding) = json["findings"].as_array().and_then(|findings| {
        findings
            .iter()
            .find(|finding| finding["rule_id"] == "dart-decimate/unused-widget-param")
    }) else {
        panic!("unused widget param finding");
    };
    finding
}

fn assert_no_widget_param_for(json: &Value, param: &str) {
    assert!(json["findings"].as_array().is_some_and(|findings| {
        findings.iter().all(|finding| {
            finding["rule_id"] != "dart-decimate/unused-widget-param"
                || finding["actions"][0]["target_symbol"]
                    .as_str()
                    .is_none_or(|symbol| !symbol.ends_with(&format!(".{param}")))
        })
    }));
}

fn assert_widget_param_for(json: &Value, target_symbol: &str) {
    assert!(json["findings"].as_array().is_some_and(|findings| {
        findings.iter().any(|finding| {
            finding["rule_id"] == "dart-decimate/unused-widget-param"
                && finding["actions"][0]["target_symbol"] == target_symbol
        })
    }));
}

fn assert_no_widget_target(json: &Value, target_symbol: &str) {
    assert!(
        json["findings"].as_array().is_some_and(|findings| {
            findings.iter().all(|finding| {
                finding["rule_id"] != "dart-decimate/unused-widget-param"
                    || finding["actions"][0]["target_symbol"] != target_symbol
            })
        }),
        "{target_symbol} should not be reported: {:?}",
        json["findings"]
    );
}

fn assert_targets_contain(targets: &[String], expected: &[&str]) {
    for target in expected {
        assert!(
            targets.iter().any(|candidate| candidate == target),
            "{target} should be unused"
        );
    }
}

fn assert_targets_do_not_contain(targets: &[String], expected: &[&str]) {
    for target in expected {
        assert!(
            targets.iter().all(|candidate| candidate != target),
            "{target} should be used"
        );
    }
}

fn unused_widget_targets(json: &Value) -> Vec<String> {
    json["findings"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|finding| finding["rule_id"] == "dart-decimate/unused-widget-param")
        .filter_map(|finding| finding["actions"][0]["target_symbol"].as_str())
        .map(str::to_owned)
        .collect()
}

fn write(fixture: &TempDir, path: &str, source: &str) -> Result<(), std::io::Error> {
    let path = fixture.path().join(path);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, source)
}
