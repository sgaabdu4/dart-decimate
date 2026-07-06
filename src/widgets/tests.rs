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
        vec!["LocalShadowCard.title", "ParameterShadowCard.title"]
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
