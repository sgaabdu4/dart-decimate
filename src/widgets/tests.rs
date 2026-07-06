use std::path::Path;

use super::*;

#[test]
fn flags_unused_stateless_widget_field_formal() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
class UserCard extends StatelessWidget {
  const UserCard({super.key, required this.title, required this.subtitle});
  final String title;
  final String subtitle;
  Widget build(BuildContext context) => Text(title);
}
";
    let unused = parse_findings(source)?.unused_params;

    assert_eq!(unused.len(), 1);
    assert_eq!(unused[0].widget_class, "UserCard");
    assert_eq!(unused[0].param_name, "subtitle");
    assert_eq!(unused[0].location.line, 3);
    Ok(())
}

#[test]
fn flags_unused_explicit_widget_constructor_params() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
class UserCard extends StatelessWidget {
  const UserCard({super.key, required String unused, required String used})
      : unused = unused,
        used = used;
  final String unused;
  final String used;
  Widget build(BuildContext context) => Text(used);
}
";
    let unused = parse_findings(source)?.unused_params;

    assert_eq!(unused.len(), 1);
    assert_eq!(unused[0].widget_class, "UserCard");
    assert_eq!(unused[0].param_name, "unused");
    assert_eq!(unused[0].location.line, 3);
    Ok(())
}

#[test]
fn respects_explicit_params_used_through_backing_fields_and_state()
-> Result<(), Box<dyn std::error::Error>> {
    let source = r"
class UserCard extends StatelessWidget {
  const UserCard({super.key, required String label}) : _label = label;
  final String _label;
  Widget build(BuildContext context) => Text(_label);
}
class CounterCard extends StatefulWidget {
  const CounterCard({super.key, required int count}) : _count = count;
  final int _count;
  State<CounterCard> createState() => _CounterCardState();
}
class _CounterCardState extends State<CounterCard> {
  Widget build(BuildContext context) => Text('${widget._count}');
}
";
    let unused = parse_findings(source)?.unused_params;

    assert!(unused.is_empty(), "{unused:?}");
    Ok(())
}

#[test]
fn flags_explicit_params_when_backing_field_is_unused() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
class UserCard extends StatelessWidget {
  const UserCard({super.key, required String subtitle}) : _subtitle = subtitle;
  final String _subtitle;
  Widget build(BuildContext context) => const SizedBox();
}
";
    let unused = parse_findings(source)?.unused_params;

    assert_eq!(unused.len(), 1);
    assert_eq!(unused[0].param_name, "subtitle");
    Ok(())
}

#[test]
fn respects_widget_and_state_usages() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
class UsedInBuild extends StatelessWidget {
  const UsedInBuild({super.key, required this.title});
  final String title;
  Widget build(BuildContext context) => Text('$title');
}
class UsedViaState extends StatefulWidget {
  const UsedViaState({super.key, required this.count});
  final int count;
  State<UsedViaState> createState() => _UsedViaStateState();
}
class _UsedViaStateState extends State<UsedViaState> {
  Widget build(BuildContext context) => Text('${widget.count}');
}
";
    let unused = parse_findings(source)?.unused_params;

    assert!(unused.is_empty(), "{unused:?}");
    Ok(())
}

#[test]
fn state_field_reads_ignore_widget_root_shadows() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
class RealStateReadPanel extends StatefulWidget {
  const RealStateReadPanel({super.key, required this.title});
  final String title;
  State<RealStateReadPanel> createState() => _RealStateReadPanelState();
}
class _RealStateReadPanelState extends State<RealStateReadPanel> {
  Widget build(BuildContext context) => Text(widget.title);
}
class DidUpdateWidgetPanel extends StatefulWidget {
  const DidUpdateWidgetPanel({super.key, required this.title});
  final String title;
  State<DidUpdateWidgetPanel> createState() => _DidUpdateWidgetPanelState();
}
class _DidUpdateWidgetPanelState extends State<DidUpdateWidgetPanel> {
  void didUpdateWidget(covariant DidUpdateWidgetPanel oldWidget) {
    super.didUpdateWidget(oldWidget);
    Text(oldWidget.title);
  }
  Widget build(BuildContext context) => const SizedBox();
}
class LocalShadowStatePanel extends StatefulWidget {
  const LocalShadowStatePanel({super.key, required this.title});
  final String title;
  State<LocalShadowStatePanel> createState() => _LocalShadowStatePanelState();
}
class _LocalShadowStatePanelState extends State<LocalShadowStatePanel> {
  Widget build(BuildContext context) {
    final widget = const LocalShadowStatePanel(title: 'local');
    return Text(widget.title);
  }
}
class CallbackShadowStatePanel extends StatefulWidget {
  const CallbackShadowStatePanel({super.key, required this.title});
  final String title;
  State<CallbackShadowStatePanel> createState() => _CallbackShadowStatePanelState();
}
class _CallbackShadowStatePanelState extends State<CallbackShadowStatePanel> {
  Widget build(BuildContext context) {
    return [const CallbackShadowStatePanel(title: 'local')]
        .map((widget) => Text(widget.title))
        .first;
  }
}
class HeaderShadowStatePanel extends StatefulWidget {
  const HeaderShadowStatePanel({super.key, required this.title});
  final String title;
  State<HeaderShadowStatePanel> createState() => _HeaderShadowStatePanelState();
}
class _HeaderShadowStatePanelState extends State<HeaderShadowStatePanel> {
  Widget build(BuildContext context) {
    for (final widget in [const HeaderShadowStatePanel(title: 'local')]) {
      Text(widget.title);
    }
    return const SizedBox();
  }
}
class PatternShadowStatePanel extends StatefulWidget {
  const PatternShadowStatePanel({super.key, required this.title});
  final String title;
  State<PatternShadowStatePanel> createState() => _PatternShadowStatePanelState();
}
class _PatternShadowStatePanelState extends State<PatternShadowStatePanel> {
  Widget build(BuildContext context) {
    final record = (widget: const PatternShadowStatePanel(title: 'local'));
    final (:widget) = record;
    return Text(widget.title);
  }
}
class CaseShadowStatePanel extends StatefulWidget {
  const CaseShadowStatePanel({super.key, required this.title});
  final String title;
  State<CaseShadowStatePanel> createState() => _CaseShadowStatePanelState();
}
class _CaseShadowStatePanelState extends State<CaseShadowStatePanel> {
  Widget build(BuildContext context) {
    if ((widget: const CaseShadowStatePanel(title: 'local')) case (:final widget)) {
      Text(widget.title);
    }
    return const SizedBox();
  }
}
class OldWidgetLocalShadowPanel extends StatefulWidget {
  const OldWidgetLocalShadowPanel({super.key, required this.title});
  final String title;
  State<OldWidgetLocalShadowPanel> createState() => _OldWidgetLocalShadowPanelState();
}
class _OldWidgetLocalShadowPanelState extends State<OldWidgetLocalShadowPanel> {
  void didUpdateWidget(covariant OldWidgetLocalShadowPanel oldWidget) {
    super.didUpdateWidget(oldWidget);
    final oldWidget = const OldWidgetLocalShadowPanel(title: 'local');
    Text(oldWidget.title);
  }
  Widget build(BuildContext context) => const SizedBox();
}
";
    let targets = unused_param_targets(parse_findings(source)?.unused_params);

    assert_eq!(
        targets,
        vec![
            "LocalShadowStatePanel.title",
            "CallbackShadowStatePanel.title",
            "HeaderShadowStatePanel.title",
            "PatternShadowStatePanel.title",
            "CaseShadowStatePanel.title",
            "OldWidgetLocalShadowPanel.title"
        ]
    );
    Ok(())
}

#[test]
fn direct_field_reads_ignore_local_and_parameter_shadows() -> Result<(), Box<dyn std::error::Error>>
{
    let source = r"
class LocalShadowCard extends StatelessWidget {
  const LocalShadowCard({super.key, required this.title});
  final String title;
  Widget build(BuildContext context) {
    const title = 'local';
    return Text(title);
  }
}
class ParameterShadowCard extends StatelessWidget {
  const ParameterShadowCard({super.key, required this.title});
  final String title;
  Widget label(String title) => Text(title);
  Widget build(BuildContext context) => label('local');
}
class CallbackParameterShadowCard extends StatelessWidget {
  const CallbackParameterShadowCard({super.key, required this.title});
  final String title;
  Widget build(BuildContext context) {
    return ['local'].map((title) => Text(title)).first;
  }
}
class DirectFieldCard extends StatelessWidget {
  const DirectFieldCard({super.key, required this.title});
  final String title;
  Widget build(BuildContext context) => Text(title);
}
class LaterLocalShadowCard extends StatelessWidget {
  const LaterLocalShadowCard({super.key, required this.title});
  final String title;
  Widget build(BuildContext context) {
    final child = Text(title);
    const title = 'local';
    return child;
  }
}
class LaterCallbackShadowCard extends StatelessWidget {
  const LaterCallbackShadowCard({super.key, required this.title});
  final String title;
  Widget build(BuildContext context) {
    final child = Text(title);
    ['local'].map((title) => Text(title)).toList();
    return child;
  }
}
class EarlierBlockShadowCard extends StatelessWidget {
  const EarlierBlockShadowCard({super.key, required this.title});
  final String title;
  Widget build(BuildContext context) {
    if (true) {
      const title = 'local';
      Text(title);
    }
    return Text(title);
  }
}
class EarlierFunctionShadowCard extends StatelessWidget {
  const EarlierFunctionShadowCard({super.key, required this.title});
  final String title;
  Widget build(BuildContext context) {
    void label() {
      const title = 'local';
      Text(title);
    }
    label();
    return Text(title);
  }
}
class ElseBranchShadowCard extends StatelessWidget {
  const ElseBranchShadowCard({super.key, required this.title});
  final String title;
  Widget build(BuildContext context, bool useLocal) {
    if (useLocal) {
      const title = 'local';
      return Text(title);
    } else {
      return Text(title);
    }
  }
}
class FunctionTypedSignatureShadowCard extends StatelessWidget {
  const FunctionTypedSignatureShadowCard({super.key, required this.title});
  final String title;
  Widget build(BuildContext context, String Function(String title) format) => Text(title);
}
";
    let unused = parse_findings(source)?.unused_params;
    let targets = unused
        .iter()
        .map(|param| format!("{}.{}", param.widget_class, param.param_name))
        .collect::<Vec<_>>();

    assert_eq!(
        targets,
        vec![
            "LocalShadowCard.title",
            "ParameterShadowCard.title",
            "CallbackParameterShadowCard.title"
        ]
    );
    Ok(())
}

#[test]
fn direct_field_reads_ignore_for_in_and_catch_header_shadows()
-> Result<(), Box<dyn std::error::Error>> {
    let source = r"
class ForInShadowCard extends StatelessWidget {
  const ForInShadowCard({super.key, required this.title});
  final String title;
  Widget build(BuildContext context) {
    for (final title in ['local']) {
      Text(title);
    }
    return const SizedBox();
  }
}
class CatchExceptionShadowCard extends StatelessWidget {
  const CatchExceptionShadowCard({super.key, required this.title});
  final String title;
  Widget build(BuildContext context) {
    try {
      throw Exception();
    } catch (title) {
      return Text(title);
    }
    return const SizedBox();
  }
}
class CatchStackShadowCard extends StatelessWidget {
  const CatchStackShadowCard({super.key, required this.title});
  final String title;
  Widget build(BuildContext context) {
    try {
      throw Exception();
    } catch (error, title) {
      return Text(title);
    }
    return const SizedBox();
  }
}
class CatchFinallyFieldCard extends StatelessWidget {
  const CatchFinallyFieldCard({super.key, required this.title});
  final String title;
  Widget build(BuildContext context) {
    try {
      throw Exception();
    } catch (title) {
      Text(title);
    } finally {
      Text(title);
    }
    return const SizedBox();
  }
}
class DirectFieldCard extends StatelessWidget {
  const DirectFieldCard({super.key, required this.title});
  final String title;
  Widget build(BuildContext context) => Text(title);
}
";
    let targets = unused_param_targets(parse_findings(source)?.unused_params);

    assert_eq!(
        targets,
        vec![
            "ForInShadowCard.title",
            "CatchExceptionShadowCard.title",
            "CatchStackShadowCard.title"
        ]
    );
    Ok(())
}

#[test]
fn forwarded_usage_requires_exact_constructor_invocation() -> Result<(), Box<dyn std::error::Error>>
{
    let source = r"
class ForwardingPanel extends StatefulWidget {
  const ForwardingPanel({super.key, required this.items});
  final List<String> items;
  State<ForwardingPanel> createState() => _ForwardingPanelState();
}

class _ForwardingPanelState extends State<ForwardingPanel> {
  Widget build(BuildContext context) {
    final ignored = NotForwardedViewData.fromOwner(source: widget);
    final label = 'ForwardedViewData.fromOwner(source: widget)';
    return Text('$ignored $label');
  }
}

class ForwardedViewData {
  ForwardedViewData.fromOwner({required ForwardingPanel source})
      : items = source.items;
  final List<String> items;
}

class NotForwardedViewData {
  NotForwardedViewData.fromOwner({required ForwardingPanel source});
}
";
    let targets = unused_param_targets(parse_findings(source)?.unused_params);

    assert_eq!(targets, vec!["ForwardingPanel.items"]);
    Ok(())
}

#[test]
fn forwarded_usage_ignores_nested_parameter_shadows() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
class ForwardingPanel extends StatefulWidget {
  const ForwardingPanel({super.key, required this.items});
  final List<String> items;
  State<ForwardingPanel> createState() => _ForwardingPanelState();
}

class _ForwardingPanelState extends State<ForwardingPanel> {
  Widget build(BuildContext context) {
    final data = ForwardedViewData.fromOwner(source: widget);
    return Text(data.label);
  }
}

class ForwardedViewData {
  ForwardedViewData.fromOwner({required ForwardingPanel source})
      : label = [const ForwardingPanel(items: ['fallback'])]
            .map((source) => source.items.join(','))
            .first;
  final String label;
}
";
    let targets = unused_param_targets(parse_findings(source)?.unused_params);

    assert_eq!(targets, vec!["ForwardingPanel.items"]);
    Ok(())
}

#[test]
fn forwarded_usage_ignores_forwarder_local_shadows() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
class BlockLocalForwardingPanel extends StatefulWidget {
  const BlockLocalForwardingPanel({super.key, required this.items});
  final List<String> items;
  State<BlockLocalForwardingPanel> createState() => _BlockLocalForwardingPanelState();
}

class _BlockLocalForwardingPanelState extends State<BlockLocalForwardingPanel> {
  Widget build(BuildContext context) {
    final data = ForwardedViewData.fromBlockLocal(source: widget);
    return Text(data.label);
  }
}

class PatternLocalForwardingPanel extends StatefulWidget {
  const PatternLocalForwardingPanel({super.key, required this.items});
  final List<String> items;
  State<PatternLocalForwardingPanel> createState() => _PatternLocalForwardingPanelState();
}

class _PatternLocalForwardingPanelState extends State<PatternLocalForwardingPanel> {
  Widget build(BuildContext context) {
    final data = ForwardedViewData.fromPatternLocal(source: widget);
    return Text(data.label);
  }
}

class ForwardedViewData {
  ForwardedViewData.fromBlockLocal({required BlockLocalForwardingPanel source}) {
    final local = const BlockLocalForwardingPanel(items: ['local']);
    {
      final source = local;
      label = source.items.join(',');
    }
  }
  ForwardedViewData.fromPatternLocal({required PatternLocalForwardingPanel source}) {
    final record = (source: const PatternLocalForwardingPanel(items: ['local']));
    final (:source) = record;
    label = source.items.join(',');
  }
  late final String label;
}
";
    let targets = unused_param_targets(parse_findings(source)?.unused_params);

    assert_eq!(
        targets,
        vec![
            "BlockLocalForwardingPanel.items",
            "PatternLocalForwardingPanel.items"
        ]
    );
    Ok(())
}

#[test]
fn forwarded_usage_counts_root_aliases() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
class ForwardingPanel extends StatefulWidget {
  const ForwardingPanel({super.key, required this.items, required this.unused});
  final List<String> items;
  final String unused;
  State<ForwardingPanel> createState() => _ForwardingPanelState();
}

class _ForwardingPanelState extends State<ForwardingPanel> {
  Widget build(BuildContext context) {
    final owner = widget;
    final data = ForwardedViewData.fromOwner(source: owner);
    return Text(data.label);
  }
}

class ForwardedViewData {
  ForwardedViewData.fromOwner({required ForwardingPanel source})
      : label = source.items.join(',');
  final String label;
}
";
    let targets = unused_param_targets(parse_findings(source)?.unused_params);

    assert_eq!(targets, vec!["ForwardingPanel.unused"]);
    Ok(())
}

#[test]
fn forwarded_usage_ignores_root_shadows_at_invocation() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
class LocalShadowForwardingPanel extends StatefulWidget {
  const LocalShadowForwardingPanel({super.key, required this.items});
  final List<String> items;
  State<LocalShadowForwardingPanel> createState() => _LocalShadowForwardingPanelState();
}

class _LocalShadowForwardingPanelState extends State<LocalShadowForwardingPanel> {
  Widget build(BuildContext context) {
    final widget = const LocalShadowForwardingPanel(items: ['local']);
    final data = ForwardedViewData.fromLocal(source: widget);
    return Text(data.label);
  }
}

class CallbackShadowForwardingPanel extends StatefulWidget {
  const CallbackShadowForwardingPanel({super.key, required this.items});
  final List<String> items;
  State<CallbackShadowForwardingPanel> createState() => _CallbackShadowForwardingPanelState();
}

class _CallbackShadowForwardingPanelState extends State<CallbackShadowForwardingPanel> {
  Widget build(BuildContext context) {
    final data = [const CallbackShadowForwardingPanel(items: ['local'])]
        .map((widget) => ForwardedViewData.fromCallback(source: widget))
        .first;
    return Text(data.label);
  }
}

class CallbackAliasShadowForwardingPanel extends StatefulWidget {
  const CallbackAliasShadowForwardingPanel({super.key, required this.items});
  final List<String> items;
  State<CallbackAliasShadowForwardingPanel> createState() => _CallbackAliasShadowForwardingPanelState();
}

class _CallbackAliasShadowForwardingPanelState extends State<CallbackAliasShadowForwardingPanel> {
  Widget build(BuildContext context) {
    final data = [const CallbackAliasShadowForwardingPanel(items: ['local'])]
        .map((widget) {
          final alias = widget;
          return ForwardedViewData.fromCallbackAlias(source: alias);
        })
        .first;
    return Text(data.label);
  }
}

class ForwardedViewData {
  ForwardedViewData.fromLocal({required LocalShadowForwardingPanel source})
      : label = source.items.join(',');
  ForwardedViewData.fromCallback({required CallbackShadowForwardingPanel source})
      : label = source.items.join(',');
  ForwardedViewData.fromCallbackAlias({required CallbackAliasShadowForwardingPanel source})
      : label = source.items.join(',');
  final String label;
}
";
    let targets = unused_param_targets(parse_findings(source)?.unused_params);

    assert_eq!(
        targets,
        vec![
            "LocalShadowForwardingPanel.items",
            "CallbackShadowForwardingPanel.items",
            "CallbackAliasShadowForwardingPanel.items"
        ]
    );
    Ok(())
}

#[test]
fn forwarded_usage_ignores_header_root_shadows() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
class ForHeaderForwardingPanel extends StatefulWidget {
  const ForHeaderForwardingPanel({super.key, required this.items});
  final List<String> items;
  State<ForHeaderForwardingPanel> createState() => _ForHeaderForwardingPanelState();
}

class _ForHeaderForwardingPanelState extends State<ForHeaderForwardingPanel> {
  Widget build(BuildContext context) {
    for (final widget in [const ForHeaderForwardingPanel(items: ['local'])]) {
      final data = ForwardedViewData.fromForHeader(source: widget);
      return Text(data.label);
    }
    return const SizedBox();
  }
}

class CatchHeaderForwardingPanel extends StatefulWidget {
  const CatchHeaderForwardingPanel({super.key, required this.items});
  final List<String> items;
  State<CatchHeaderForwardingPanel> createState() => _CatchHeaderForwardingPanelState();
}

class _CatchHeaderForwardingPanelState extends State<CatchHeaderForwardingPanel> {
  Widget build(BuildContext context) {
    try {
      throw Object();
    } catch (widget) {
      final alias = widget;
      final data = ForwardedViewData.fromCatchHeader(source: alias);
      return Text(data.label);
    }
    return const SizedBox();
  }
}

class ForwardedViewData {
  ForwardedViewData.fromForHeader({required ForHeaderForwardingPanel source})
      : label = source.items.join(',');
  ForwardedViewData.fromCatchHeader({required CatchHeaderForwardingPanel source})
      : label = source.items.join(',');
  final String label;
}
";
    let targets = unused_param_targets(parse_findings(source)?.unused_params);

    assert_eq!(
        targets,
        vec![
            "ForHeaderForwardingPanel.items",
            "CatchHeaderForwardingPanel.items"
        ]
    );
    Ok(())
}

#[test]
fn forwarded_usage_ignores_local_function_root_shadows() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
class LocalFunctionForwardingPanel extends StatefulWidget {
  const LocalFunctionForwardingPanel({super.key, required this.items});
  final List<String> items;
  State<LocalFunctionForwardingPanel> createState() => _LocalFunctionForwardingPanelState();
}

class _LocalFunctionForwardingPanelState extends State<LocalFunctionForwardingPanel> {
  Widget build(BuildContext context) {
    void widget() {}
    final data = ForwardedViewData.fromLocalFunction(source: widget);
    return Text(data.label);
  }
}

class ForwardedViewData {
  ForwardedViewData.fromLocalFunction({required LocalFunctionForwardingPanel source})
      : label = source.items.join(',');
  final String label;
}
";
    let targets = unused_param_targets(parse_findings(source)?.unused_params);

    assert_eq!(targets, vec!["LocalFunctionForwardingPanel.items"]);
    Ok(())
}

#[test]
fn object_pattern_usage_ignores_shadowed_root_aliases() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
class AliasPatternPanel extends StatefulWidget {
  const AliasPatternPanel({super.key, required this.used, required this.unused});
  final String used;
  final String unused;
  State<AliasPatternPanel> createState() => _AliasPatternPanelState();
}

class _AliasPatternPanelState extends State<AliasPatternPanel> {
  Widget build(BuildContext context) {
    return Text([const AliasPatternPanel(used: 'local', unused: 'local')]
        .map((widget) {
          final alias = widget;
          final AliasPatternPanel(:used) = alias;
          return used;
        })
        .first);
  }
}
";
    let targets = unused_param_targets(parse_findings(source)?.unused_params);

    assert_eq!(
        targets,
        vec!["AliasPatternPanel.unused", "AliasPatternPanel.used"]
    );
    Ok(())
}

#[test]
fn object_pattern_usage_ignores_header_root_shadows() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
class HeaderPatternPanel extends StatefulWidget {
  const HeaderPatternPanel({super.key, required this.used, required this.unused});
  final String used;
  final String unused;
  State<HeaderPatternPanel> createState() => _HeaderPatternPanelState();
}

class _HeaderPatternPanelState extends State<HeaderPatternPanel> {
  Widget build(BuildContext context) {
    for (final widget in [const HeaderPatternPanel(used: 'local', unused: 'local')]) {
      final HeaderPatternPanel(:used) = widget;
      return Text(used);
    }
    return const SizedBox();
  }
}
";
    let targets = unused_param_targets(parse_findings(source)?.unused_params);

    assert_eq!(
        targets,
        vec!["HeaderPatternPanel.unused", "HeaderPatternPanel.used"]
    );
    Ok(())
}

#[test]
fn object_pattern_usage_ignores_local_function_root_shadows()
-> Result<(), Box<dyn std::error::Error>> {
    let source = r"
class LocalFunctionPatternPanel extends StatefulWidget {
  const LocalFunctionPatternPanel({super.key, required this.used, required this.unused});
  final String used;
  final String unused;
  State<LocalFunctionPatternPanel> createState() => _LocalFunctionPatternPanelState();
}

class _LocalFunctionPatternPanelState extends State<LocalFunctionPatternPanel> {
  Widget build(BuildContext context) {
    void widget() {}
    final LocalFunctionPatternPanel(:used) = widget;
    return Text(used);
  }
}
";
    let targets = unused_param_targets(parse_findings(source)?.unused_params);

    assert_eq!(
        targets,
        vec![
            "LocalFunctionPatternPanel.unused",
            "LocalFunctionPatternPanel.used"
        ]
    );
    Ok(())
}

#[test]
fn object_pattern_usage_counts_root_aliases() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
class AliasPatternPanel extends StatefulWidget {
  const AliasPatternPanel({super.key, required this.used, required this.unused});
  final String used;
  final String unused;
  State<AliasPatternPanel> createState() => _AliasPatternPanelState();
}

class _AliasPatternPanelState extends State<AliasPatternPanel> {
  Widget build(BuildContext context) {
    final owner = widget;
    final AliasPatternPanel(:used) = owner;
    return Text(used);
  }
}
";
    let targets = unused_param_targets(parse_findings(source)?.unused_params);

    assert_eq!(targets, vec!["AliasPatternPanel.unused"]);
    Ok(())
}

#[test]
fn factory_forwarders_count_fields_read_from_widget_parameter()
-> Result<(), Box<dyn std::error::Error>> {
    let source = r"
class FactoryForwardingPanel extends StatelessWidget {
  const FactoryForwardingPanel({super.key, required this.title, required this.unused});
  final String title;
  final String unused;
  Widget build(BuildContext context) {
    final data = ForwardedTitleData.fromOwner(source: this);
    return Text(data.title);
  }
}

class ForwardedTitleData {
  factory ForwardedTitleData.fromOwner({required FactoryForwardingPanel source}) {
    return ForwardedTitleData._(source.title);
  }
  ForwardedTitleData._(this.title);
  final String title;
}
";
    let targets = unused_param_targets(parse_findings(source)?.unused_params);

    assert_eq!(targets, vec!["FactoryForwardingPanel.unused"]);
    Ok(())
}

#[test]
fn forwarders_count_object_pattern_fields_read_from_widget_parameter()
-> Result<(), Box<dyn std::error::Error>> {
    let source = r"
class PatternForwardingPanel extends StatelessWidget {
  const PatternForwardingPanel({super.key, required this.title, required this.unused});
  final String title;
  final String unused;
  Widget build(BuildContext context) {
    final data = ForwardedTitleData.fromOwner(source: this);
    return Text(data.title);
  }
}

class ForwardedTitleData {
  ForwardedTitleData.fromOwner({required PatternForwardingPanel source}) {
    final PatternForwardingPanel(:title) = source;
    this.title = title;
  }
  late final String title;
}
";
    let targets = unused_param_targets(parse_findings(source)?.unused_params);

    assert_eq!(targets, vec!["PatternForwardingPanel.unused"]);
    Ok(())
}

#[test]
fn forwarders_match_generic_widget_parameter_types() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
class GenericForwarderParamPanel<T> extends StatelessWidget {
  const GenericForwarderParamPanel({super.key, required this.title, required this.unused});
  final String title;
  final String unused;
  Widget build(BuildContext context) {
    final data = GenericForwardedTitleData<T>.fromOwner(source: this);
    return Text(data.title);
  }
}

class GenericForwardedTitleData<T> {
  GenericForwardedTitleData.fromOwner({required GenericForwarderParamPanel<T> source})
      : title = source.title;
  final String title;
}
";
    let targets = unused_param_targets(parse_findings(source)?.unused_params);

    assert_eq!(targets, vec!["GenericForwarderParamPanel.unused"]);
    Ok(())
}

#[test]
fn forwarded_usage_matches_generic_const_forwarder_invocations()
-> Result<(), Box<dyn std::error::Error>> {
    let source = r"
class GenericForwardingPanel extends StatelessWidget {
  const GenericForwardingPanel({super.key, required this.title, required this.unused});
  final String title;
  final String unused;
  Widget build(BuildContext context) {
    final data = const ForwardedTitleData<String>.fromOwner(source: this);
    return Text(data.title);
  }
}

class ForwardedTitleData<T> {
  const ForwardedTitleData.fromOwner({required GenericForwardingPanel source})
      : title = source.title;
  final String title;
}
";
    let targets = unused_param_targets(parse_findings(source)?.unused_params);

    assert_eq!(targets, vec!["GenericForwardingPanel.unused"]);
    Ok(())
}

#[test]
fn forwarders_ignore_nested_callback_formals_for_positional_indexes()
-> Result<(), Box<dyn std::error::Error>> {
    let source = r"
class ForwarderArityPanel extends StatelessWidget {
  const ForwarderArityPanel({super.key, required this.title, required this.unused});
  final String title;
  final String unused;
  Widget build(BuildContext context) {
    String format(ForwarderArityPanel panel) => panel.title;
    final data = ForwardedTitleData.fromOwner(format, this);
    return Text(data.title);
  }
}

class ForwardedTitleData {
  ForwardedTitleData.fromOwner(
    String Function(ForwarderArityPanel panel) format,
    ForwarderArityPanel source,
  ) : title = source.title;
  final String title;
}
";
    let targets = unused_param_targets(parse_findings(source)?.unused_params);

    assert_eq!(targets, vec!["ForwarderArityPanel.unused"]);
    Ok(())
}

#[test]
fn object_pattern_usage_counts_paired_nested_record_roots() -> Result<(), Box<dyn std::error::Error>>
{
    let source = r"
class NestedRecordRootPanel extends StatelessWidget {
  const NestedRecordRootPanel({super.key, required this.used, required this.unused});
  final String used;
  final String unused;
  Widget build(BuildContext context) {
    final (NestedRecordRootPanel(:used), _) = (this, Object());
    return Text(used);
  }
}

class NestedRecordStringPanel extends StatelessWidget {
  const NestedRecordStringPanel({super.key, required this.used, required this.unused});
  final String used;
  final String unused;
  Widget build(BuildContext context) {
    final (_, NestedRecordStringPanel(:used)) = ('a,b', this);
    return Text(used);
  }
}

class NestedRecordOtherPanel extends StatelessWidget {
  const NestedRecordOtherPanel({super.key, required this.used, required this.unused});
  final String used;
  final String unused;
  Widget build(BuildContext context) {
    final (NestedRecordOtherPanel(:used), _) =
        (const NestedRecordOtherPanel(used: 'other', unused: 'other'), this);
    return Text(used);
  }
}
";
    let mut targets = unused_param_targets(parse_findings(source)?.unused_params);
    targets.sort();

    assert_eq!(
        targets,
        vec![
            "NestedRecordOtherPanel.unused",
            "NestedRecordOtherPanel.used",
            "NestedRecordRootPanel.unused",
            "NestedRecordStringPanel.unused"
        ]
    );
    Ok(())
}

#[test]
fn object_pattern_usage_counts_paired_nested_case_roots() -> Result<(), Box<dyn std::error::Error>>
{
    let source = r"
class NestedIfCaseRootPanel extends StatelessWidget {
  const NestedIfCaseRootPanel({super.key, required this.used, required this.unused});
  final String used;
  final String unused;
  Widget build(BuildContext context) {
    if ((this, Object()) case (NestedIfCaseRootPanel(:used), _)) {
      return Text(used);
    }
    return const SizedBox();
  }
}

class NestedSwitchCaseRootPanel extends StatelessWidget {
  const NestedSwitchCaseRootPanel({super.key, required this.used, required this.unused});
  final String used;
  final String unused;
  Widget build(BuildContext context) {
    return switch ((Object(), this)) {
      (_, NestedSwitchCaseRootPanel(:used)) => Text(used),
    };
  }
}

class NestedCaseOtherPanel extends StatelessWidget {
  const NestedCaseOtherPanel({super.key, required this.used, required this.unused});
  final String used;
  final String unused;
  Widget build(BuildContext context) {
    if ((Object(), const NestedCaseOtherPanel(used: 'other', unused: 'other'))
        case (_, NestedCaseOtherPanel(:used))) {
      return Text(used);
    }
    return const SizedBox();
  }
}
";
    let mut targets = unused_param_targets(parse_findings(source)?.unused_params);
    targets.sort();

    assert_eq!(
        targets,
        vec![
            "NestedCaseOtherPanel.unused",
            "NestedCaseOtherPanel.used",
            "NestedIfCaseRootPanel.unused",
            "NestedSwitchCaseRootPanel.unused"
        ]
    );
    Ok(())
}

#[test]
fn bare_object_pattern_helper_calls_ignore_mixin_member_shadows()
-> Result<(), Box<dyn std::error::Error>> {
    let source = r"
class MixinShadowHelperWidget extends StatelessWidget with HelperMixin {
  const MixinShadowHelperWidget({super.key, required this.used, required this.unused});
  final String used;
  final String unused;
  Widget build(BuildContext context) => Text(mixinShadowHelper(this));
}

mixin HelperMixin {
  String mixinShadowHelper(MixinShadowHelperWidget widget) => 'member';
}

String mixinShadowHelper(MixinShadowHelperWidget widget) {
  final MixinShadowHelperWidget(:used) = widget;
  return used;
}
";
    let mut targets = unused_param_targets(parse_findings(source)?.unused_params);
    targets.sort();

    assert_eq!(
        targets,
        vec![
            "MixinShadowHelperWidget.unused",
            "MixinShadowHelperWidget.used"
        ]
    );
    Ok(())
}

#[test]
fn bare_object_pattern_helper_calls_ignore_unresolved_interface_shadows()
-> Result<(), Box<dyn std::error::Error>> {
    let source = r"
class ImportedInterfaceShadowWidget extends StatelessWidget implements ImportedHelperOwner {
  const ImportedInterfaceShadowWidget({super.key, required this.used, required this.unused});
  final String used;
  final String unused;
  Widget build(BuildContext context) => Text(importedShadowHelper(this));
}

String importedShadowHelper(ImportedInterfaceShadowWidget widget) {
  final ImportedInterfaceShadowWidget(:used) = widget;
  return used;
}
";
    let mut targets = unused_param_targets(parse_findings(source)?.unused_params);
    targets.sort();

    assert_eq!(
        targets,
        vec![
            "ImportedInterfaceShadowWidget.unused",
            "ImportedInterfaceShadowWidget.used"
        ]
    );
    Ok(())
}

#[test]
fn bare_object_pattern_helper_calls_ignore_prefixed_inherited_shadows()
-> Result<(), Box<dyn std::error::Error>> {
    let source = r"
import 'base_screen.dart' as pkg;

class BaseScreen extends StatelessWidget {}

class PrefixedInheritedShadowWidget extends StatelessWidget with pkg.BaseScreen {
  const PrefixedInheritedShadowWidget({super.key, required this.used, required this.unused});
  final String used;
  final String unused;
  Widget build(BuildContext context) => Text(prefixedShadowHelper(this));
}

String prefixedShadowHelper(PrefixedInheritedShadowWidget widget) {
  final PrefixedInheritedShadowWidget(:used) = widget;
  return used;
}
";
    let mut targets = unused_param_targets(parse_findings(source)?.unused_params);
    targets.sort();

    assert_eq!(
        targets,
        vec![
            "PrefixedInheritedShadowWidget.unused",
            "PrefixedInheritedShadowWidget.used"
        ]
    );
    Ok(())
}

#[test]
fn object_pattern_helpers_accept_prefixed_flutter_framework_superclasses()
-> Result<(), Box<dyn std::error::Error>> {
    let source = r"
import 'package:flutter/widgets.dart' as f;

class PrefixedFrameworkHelperWidget extends f.StatelessWidget {
  const PrefixedFrameworkHelperWidget({super.key, required this.used, required this.unused});
  final String used;
  final String unused;
  f.Widget build(f.BuildContext context) => f.Text(prefixedFrameworkHelper(this));
}

String prefixedFrameworkHelper(PrefixedFrameworkHelperWidget widget) {
  final PrefixedFrameworkHelperWidget(:used) = widget;
  return used;
}
";
    let targets = unused_param_targets(parse_findings(source)?.unused_params);

    assert_eq!(targets, vec!["PrefixedFrameworkHelperWidget.unused"]);
    Ok(())
}

#[test]
fn object_pattern_helpers_ignore_unresolved_prefixed_framework_names()
-> Result<(), Box<dyn std::error::Error>> {
    let source = r"
import 'package:fake/widgets.dart' as pkg;

class PrefixedFakeFrameworkWidget extends pkg.StatelessWidget {
  const PrefixedFakeFrameworkWidget({super.key, required this.used, required this.unused});
  final String used;
  final String unused;
  Widget build(BuildContext context) => Text(prefixedFakeFrameworkHelper(this));
}

String prefixedFakeFrameworkHelper(PrefixedFakeFrameworkWidget widget) {
  final PrefixedFakeFrameworkWidget(:used) = widget;
  return used;
}
";
    let mut targets = unused_param_targets(parse_findings(source)?.unused_params);
    targets.sort();

    assert_eq!(
        targets,
        vec![
            "PrefixedFakeFrameworkWidget.unused",
            "PrefixedFakeFrameworkWidget.used"
        ]
    );
    Ok(())
}

#[test]
fn object_pattern_method_helpers_match_inherited_mixin_calls()
-> Result<(), Box<dyn std::error::Error>> {
    let source = r"
mixin InheritedPatternHelperMixin {
  String readMixinHelper(InheritedMixinHelperPanel widget) {
    final InheritedMixinHelperPanel(:used) = widget;
    return used;
  }

  String readSuperMixinHelper(SuperMixinHelperPanel widget) {
    final SuperMixinHelperPanel(:used) = widget;
    return used;
  }
}

class InheritedMixinHelperPanel extends StatelessWidget with InheritedPatternHelperMixin {
  const InheritedMixinHelperPanel({super.key, required this.used, required this.unused});
  final String used;
  final String unused;
  Widget build(BuildContext context) => Text(this.readMixinHelper(this));
}

class SuperMixinHelperPanel extends StatelessWidget with InheritedPatternHelperMixin {
  const SuperMixinHelperPanel({super.key, required this.used, required this.unused});
  final String used;
  final String unused;
  Widget build(BuildContext context) => Text(super.readSuperMixinHelper(this));
}
";
    let mut targets = unused_param_targets(parse_findings(source)?.unused_params);
    targets.sort();

    assert_eq!(
        targets,
        vec![
            "InheritedMixinHelperPanel.unused",
            "SuperMixinHelperPanel.unused"
        ]
    );
    Ok(())
}

#[test]
fn object_pattern_method_helpers_resolve_actual_dispatch_owner()
-> Result<(), Box<dyn std::error::Error>> {
    let source = r"
class ThisOverrideHelperPanel extends StatelessWidget with ThisOverrideHelperMixin {
  const ThisOverrideHelperPanel({super.key, required this.used, required this.unused});
  final String used;
  final String unused;
  Widget build(BuildContext context) => Text(this.readThisOverrideHelper(this));
  String readThisOverrideHelper(ThisOverrideHelperPanel widget) => 'override';
}

mixin ThisOverrideHelperMixin {
  String readThisOverrideHelper(ThisOverrideHelperPanel widget) {
    final ThisOverrideHelperPanel(:used) = widget;
    return used;
  }
}

class SuperOverrideLeafPanel extends StatelessWidget with SuperOverrideBaseMixin, SuperOverrideMidMixin {
  const SuperOverrideLeafPanel({super.key, required this.used, required this.unused});
  final String used;
  final String unused;
  Widget build(BuildContext context) => Text(super.readSuperOverrideHelper(this));
}

mixin SuperOverrideMidMixin {
  String readSuperOverrideHelper(SuperOverrideLeafPanel widget) => 'override';
}

mixin SuperOverrideBaseMixin {
  String readSuperOverrideHelper(SuperOverrideLeafPanel widget) {
    final SuperOverrideLeafPanel(:used) = widget;
    return used;
  }
}

class BareInheritedHelperPanel extends StatelessWidget with BareInheritedHelperMixin {
  const BareInheritedHelperPanel({super.key, required this.used, required this.unused});
  final String used;
  final String unused;
  Widget build(BuildContext context) => Text(readBareInheritedHelper(this));
}

mixin BareInheritedHelperMixin {
  String readBareInheritedHelper(BareInheritedHelperPanel widget) {
    final BareInheritedHelperPanel(:used) = widget;
    return used;
  }
}
";
    let mut targets = unused_param_targets(parse_findings(source)?.unused_params);
    targets.sort();

    assert_eq!(
        targets,
        vec![
            "BareInheritedHelperPanel.unused",
            "SuperOverrideLeafPanel.unused",
            "SuperOverrideLeafPanel.used",
            "ThisOverrideHelperPanel.unused",
            "ThisOverrideHelperPanel.used"
        ]
    );
    Ok(())
}

#[test]
fn object_pattern_helpers_match_generic_invocations_and_parameter_types()
-> Result<(), Box<dyn std::error::Error>> {
    let source = r"
class GenericHelperPanel<T> extends StatelessWidget {
  const GenericHelperPanel({super.key, required this.title, required this.unused});
  final String title;
  final String unused;
  Widget build(BuildContext context) => Text(readGenericHelper<T>(this));
}

String readGenericHelper<T>(GenericHelperPanel<T> widget) {
  final GenericHelperPanel<T>(:title) = widget;
  return title;
}
";
    let targets = unused_param_targets(parse_findings(source)?.unused_params);

    assert_eq!(targets, vec!["GenericHelperPanel.unused"]);
    Ok(())
}

#[test]
fn object_pattern_helpers_ignore_nested_callback_formals_for_positional_indexes()
-> Result<(), Box<dyn std::error::Error>> {
    let source = r"
class HelperArityPanel extends StatelessWidget {
  const HelperArityPanel({super.key, required this.title, required this.unused});
  final String title;
  final String unused;
  Widget build(BuildContext context) {
    String format(HelperArityPanel panel) => panel.title;
    return Text(readHelperArity(format, this));
  }
}

String readHelperArity(
  String Function(HelperArityPanel panel) format,
  HelperArityPanel widget,
) {
  final HelperArityPanel(:title) = widget;
  return title;
}
";
    let targets = unused_param_targets(parse_findings(source)?.unused_params);

    assert_eq!(targets, vec!["HelperArityPanel.unused"]);
    Ok(())
}

#[test]
fn root_aliases_do_not_survive_reassignment() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
class ReassignedForwardingAliasPanel extends StatelessWidget {
  const ReassignedForwardingAliasPanel({super.key, required this.used, required this.unused});
  final String used;
  final String unused;
  Widget build(BuildContext context) {
    var owner = this;
    owner = const OtherPanel(used: 'other', unused: 'other');
    final data = ForwardedAliasData.fromOwner(source: owner);
    return Text(data.used);
  }
}

class ReassignedPatternAliasPanel extends StatelessWidget {
  const ReassignedPatternAliasPanel({super.key, required this.used, required this.unused});
  final String used;
  final String unused;
  Widget build(BuildContext context) {
    var owner = this;
    owner = const OtherPanel(used: 'other', unused: 'other');
    final ReassignedPatternAliasPanel(:used) = owner;
    return Text(used);
  }
}

class OtherPanel extends StatelessWidget {
  const OtherPanel({super.key, required this.used, required this.unused});
  final String used;
  final String unused;
  Widget build(BuildContext context) => Text('$used$unused');
}

class ForwardedAliasData {
  ForwardedAliasData.fromOwner({required ReassignedForwardingAliasPanel source})
      : used = source.used;
  final String used;
}
";
    let mut targets = unused_param_targets(parse_findings(source)?.unused_params);
    targets.sort();

    assert_eq!(
        targets,
        vec![
            "ReassignedForwardingAliasPanel.unused",
            "ReassignedForwardingAliasPanel.used",
            "ReassignedPatternAliasPanel.unused",
            "ReassignedPatternAliasPanel.used",
        ]
    );
    Ok(())
}

#[test]
fn bare_object_pattern_helper_calls_ignore_header_shadows() -> Result<(), Box<dyn std::error::Error>>
{
    let source = r"
class HeaderShadowHelperPanel extends StatelessWidget {
  const HeaderShadowHelperPanel({super.key, required this.used, required this.unused});
  final String used;
  final String unused;
  Widget build(BuildContext context) {
    if (Object() case final readHeaderHelper) {
      return Text(readHeaderHelper(this));
    }
    return const SizedBox();
  }
}

String readHeaderHelper(HeaderShadowHelperPanel widget) {
  final HeaderShadowHelperPanel(:used) = widget;
  return used;
}
";
    let mut targets = unused_param_targets(parse_findings(source)?.unused_params);
    targets.sort();

    assert_eq!(
        targets,
        vec![
            "HeaderShadowHelperPanel.unused",
            "HeaderShadowHelperPanel.used"
        ]
    );
    Ok(())
}

#[test]
fn non_class_object_pattern_methods_do_not_match_bare_helper_calls()
-> Result<(), Box<dyn std::error::Error>> {
    let source = r"
class ExtensionMethodHelperPanel extends StatelessWidget {
  const ExtensionMethodHelperPanel({super.key, required this.used, required this.unused});
  final String used;
  final String unused;
  Widget build(BuildContext context) => Text(readExtensionHelper(this));
}

extension ExtensionHelper on Object {
  String readExtensionHelper(ExtensionMethodHelperPanel widget) {
    final ExtensionMethodHelperPanel(:used) = widget;
    return used;
  }
}

String readExtensionHelper(ExtensionMethodHelperPanel widget) => 'not a pattern helper';

class MixinMethodHelperPanel extends StatelessWidget {
  const MixinMethodHelperPanel({super.key, required this.used, required this.unused});
  final String used;
  final String unused;
  Widget build(BuildContext context) => Text(readMixinHelper(this));
}

mixin DetachedHelperMixin {
  String readMixinHelper(MixinMethodHelperPanel widget) {
    final MixinMethodHelperPanel(:used) = widget;
    return used;
  }
}

String readMixinHelper(MixinMethodHelperPanel widget) => 'not a pattern helper';
";
    let mut targets = unused_param_targets(parse_findings(source)?.unused_params);
    targets.sort();

    assert_eq!(
        targets,
        vec![
            "ExtensionMethodHelperPanel.unused",
            "ExtensionMethodHelperPanel.used",
            "MixinMethodHelperPanel.unused",
            "MixinMethodHelperPanel.used"
        ]
    );
    Ok(())
}

#[test]
fn recognizes_consumer_and_hook_widget_bases() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
class A extends ConsumerWidget {
  const A({super.key, required this.value});
  final String value;
  Widget build(BuildContext context, WidgetRef ref) => const SizedBox();
}
class B extends HookConsumerWidget {
  const B({super.key, required this.value});
  final String value;
  Widget build(BuildContext context, WidgetRef ref) => Text(value);
}
class C extends ConsumerStatefulWidget {
  const C({super.key, required this.value});
  final String value;
  ConsumerState<C> createState() => _CState();
}
class _CState extends ConsumerState<C> {
  Widget build(BuildContext context) => Text(oldWidget.value);
}
";
    let unused = parse_findings(source)?.unused_params;

    assert_eq!(unused.len(), 1);
    assert_eq!(unused[0].widget_class, "A");
    assert_eq!(unused[0].widget_kind, WidgetClassKind::ConsumerWidget);
    Ok(())
}

#[test]
fn flags_private_widget_classes_but_allows_private_states() -> Result<(), Box<dyn std::error::Error>>
{
    let source = r"
class PublicCard extends StatefulWidget {
  State<PublicCard> createState() => _PublicCardState();
}
class _PublicCardState extends State<PublicCard> {}
class PublicConsumer extends ConsumerStatefulWidget {
  ConsumerState<PublicConsumer> createState() => _PublicConsumerState();
}
class _PublicConsumerState extends ConsumerState<PublicConsumer> {}
class _PrivateCard extends StatelessWidget {}
class _PrivateShell extends ConsumerWidget {}
class _PrivateHook extends HookConsumerWidget {}
";
    let private_widgets = parse_findings(source)?.private_widget_classes;

    let classes = private_widgets
        .iter()
        .map(|widget| widget.widget_class.as_str())
        .collect::<Vec<_>>();
    assert_eq!(
        classes,
        vec!["_PrivateCard", "_PrivateShell", "_PrivateHook"]
    );
    Ok(())
}

#[test]
fn reports_all_private_widget_base_kinds() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
class _A extends StatelessWidget {}
class _B extends StatefulWidget {}
class _C extends ConsumerWidget {}
class _D extends ConsumerStatefulWidget {}
class _E extends HookWidget {}
class _F extends HookConsumerWidget {}
";
    let kinds = parse_findings(source)?
        .private_widget_classes
        .into_iter()
        .map(|widget| widget.widget_kind)
        .collect::<Vec<_>>();

    assert_eq!(
        kinds,
        vec![
            WidgetClassKind::StatelessWidget,
            WidgetClassKind::StatefulWidget,
            WidgetClassKind::ConsumerWidget,
            WidgetClassKind::ConsumerStatefulWidget,
            WidgetClassKind::HookWidget,
            WidgetClassKind::HookConsumerWidget,
        ]
    );
    Ok(())
}

#[test]
fn ignores_public_widgets_and_non_widget_private_classes() -> Result<(), Box<dyn std::error::Error>>
{
    let source = r"
class PublicCard extends StatelessWidget {}
class _Formatter {}
";
    let private_widgets = parse_findings(source)?.private_widget_classes;

    assert!(private_widgets.is_empty(), "{private_widgets:?}");
    Ok(())
}

#[test]
fn flags_top_level_widget_helper_functions_in_widget_files()
-> Result<(), Box<dyn std::error::Error>> {
    let source = "\
class App extends StatelessWidget {}
Widget _buildHeader(BuildContext context) => const SizedBox();
List<Widget> buildItems() => const [];
";
    let helpers = parse_findings(source)?.top_level_functions;

    assert_eq!(helpers.len(), 2);
    assert_eq!(helpers[0].function_name, "_buildHeader");
    assert_eq!(helpers[0].return_type.as_deref(), Some("Widget"));
    assert_eq!(helpers[0].location.line, 2);
    assert_eq!(helpers[1].function_name, "buildItems");
    Ok(())
}

#[test]
fn flags_top_level_helpers_in_screen_files() -> Result<(), Box<dyn std::error::Error>> {
    let source = "Widget header(BuildContext context) => const SizedBox();\n";
    let helpers = parse_findings_at("lib/screens/home_screen.dart", source)?.top_level_functions;

    assert_eq!(helpers.len(), 1);
    assert_eq!(helpers[0].function_name, "header");
    Ok(())
}

#[test]
fn does_not_flag_methods_local_functions_or_namespaces() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
class App extends StatefulWidget {}
class _AppState extends State<App> {
  Widget _buildHeader(BuildContext context) => const SizedBox();
}
abstract final class AppParts {
  static Widget header(BuildContext context) => const SizedBox();
}
void container() {
  Widget _buildLocal(BuildContext context) => const SizedBox();
}
";
    let helpers = parse_findings(source)?.top_level_functions;

    assert!(helpers.is_empty(), "{helpers:?}");
    Ok(())
}

#[test]
fn does_not_flag_main_providers_or_widget_named_configs() -> Result<(), Box<dyn std::error::Error>>
{
    let source = r"
class App extends StatelessWidget {}
void main() {}
int count(Ref ref) => 1;
String title() => 'ok';
MyWidgetConfig _buildConfig() => MyWidgetConfig();
WidgetBuilder makeBuilder() => (context) => const SizedBox();
";
    let helpers = parse_findings(source)?.top_level_functions;

    assert!(helpers.is_empty(), "{helpers:?}");
    Ok(())
}

#[test]
fn flags_widget_awaits_without_context_mounted_guard() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
class SaveButton extends StatefulWidget {
  State<SaveButton> createState() => _SaveButtonState();
}

class _SaveButtonState extends State<SaveButton> {
  Future<void> save() async {
    await doWork();
    Navigator.of(context).pop();
  }

  Future<void> guarded() async {
    await doWork();
    if (!context.mounted) return;
    Navigator.of(context).pop();
  }

  Future<void> bareMountedIsNotEnough() async {
    await doWork();
    if (!mounted) return;
    Navigator.of(context).pop();
  }
}
";
    let findings = parse_findings(source)?.missing_context_mounted_after_await;

    assert_eq!(findings.len(), 2);
    assert_eq!(findings[0].owner, "_SaveButtonState.save");
    assert_eq!(findings[0].location.line, 8);
    assert_eq!(findings[1].owner, "_SaveButtonState.bareMountedIsNotEnough");
    Ok(())
}

#[test]
fn flags_nested_widget_awaits_per_block() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
class SaveButton extends StatelessWidget {
  Future<void> save(BuildContext context, bool active) async {
    if (active) {
      await doWork();
      if (!context.mounted) return;
      Navigator.of(context).pop();
    }
    await doWork();
    Navigator.of(context).pop();
  }
}
";
    let findings = parse_findings(source)?.missing_context_mounted_after_await;

    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].owner, "SaveButton.save");
    assert_eq!(findings[0].location.line, 9);
    Ok(())
}

#[test]
fn does_not_flag_widget_awaits_without_lifecycle_use() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
class SaveButton extends StatefulWidget {
  State<SaveButton> createState() => _SaveButtonState();
}

class _SaveButtonState extends State<SaveButton> {
  Future<void> refresh() async {
    await doWork();
    await logTap();
  }
}
";
    let findings = parse_findings(source)?.missing_context_mounted_after_await;

    assert!(findings.is_empty(), "{findings:?}");
    Ok(())
}

#[test]
fn flags_widget_async_closure_awaits_and_accepts_return_guards()
-> Result<(), Box<dyn std::error::Error>> {
    let source = r"
class SaveButton extends StatelessWidget {
  Widget build(BuildContext context) {
    return ElevatedButton(
      onPressed: () async {
        await doWork();
        Navigator.of(context).pop();
      },
      onLongPress: () async {
        await logTap();
      },
      child: const Text('Save'),
    );
  }

  Future<bool> guardedFalse(BuildContext context) async {
    await doWork();
    if (!context.mounted) return false;
    return true;
  }

  Future<String?> guardedNull(BuildContext context) async {
    await doWork();
    if (!context.mounted) return null;
    return 'ok';
  }

  Future<void> guardedBraced(BuildContext context) async {
    await doWork();
    if (!context.mounted) {
      return;
    }
    Navigator.of(context).pop();
  }
}
";
    let findings = parse_findings(source)?.missing_context_mounted_after_await;

    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].owner, "SaveButton.build");
    assert_eq!(findings[0].location.line, 6);
    Ok(())
}

#[test]
fn flags_expression_bodied_lifecycle_awaits() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
class SaveButton extends StatelessWidget {
  Future<void> save(BuildContext context) async =>
      Navigator.of(context).pop(await doWork());
}
";
    let findings = parse_findings(source)?;

    assert_eq!(findings.missing_context_mounted_after_await.len(), 1);
    assert_eq!(
        findings.missing_context_mounted_after_await[0].owner,
        "SaveButton.save"
    );
    Ok(())
}

fn parse_findings(source: &str) -> Result<FileWidgetFindings, WidgetAnalysisError> {
    parse_findings_at("lib/widgets.dart", source)
}

fn parse_findings_at(path: &str, source: &str) -> Result<FileWidgetFindings, WidgetAnalysisError> {
    let path = Path::new(path);
    let parsed = parse_tree(path, source)?;
    Ok(findings_in_source(
        path,
        parsed.tree().root_node(),
        parsed.source(),
    ))
}

fn unused_param_targets(unused: Vec<UnusedWidgetParam>) -> Vec<String> {
    unused
        .into_iter()
        .map(|param| format!("{}.{}", param.widget_class, param.param_name))
        .collect()
}
