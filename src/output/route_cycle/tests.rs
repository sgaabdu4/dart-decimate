use std::collections::BTreeSet;

use super::helper_has_typed_route_navigation_call;

fn typed_route_navigation_found(source: &str) -> Result<bool, Box<dyn std::error::Error>> {
    let parsed = crate::dart_parser::parse_dart_source_lossy("helper.dart".as_ref(), source)?;
    let route_classes = BTreeSet::from(["HomeRoute".to_owned()]);
    Ok(helper_has_typed_route_navigation_call(
        parsed.tree().root_node(),
        parsed.source(),
        &route_classes,
    ))
}

#[test]
fn state_context_resolves_through_local_state_base_class() -> Result<(), Box<dyn std::error::Error>>
{
    let source = r"
import 'package:flutter/widgets.dart';

class BaseState<T> extends State<T> {}

class ScreenState extends BaseState<Screen> {
  void open() {
    context.go(const HomeRoute().location);
    const HomeRoute().go(this.context);
  }
}
";

    assert!(typed_route_navigation_found(source)?);
    Ok(())
}

#[test]
fn state_context_resolves_through_local_consumer_state_base_class()
-> Result<(), Box<dyn std::error::Error>> {
    let source = r"
import 'package:flutter_riverpod/flutter_riverpod.dart';

class BaseState<T> extends ConsumerState<T> {}

class ScreenState extends BaseState<Screen> {
  void open() {
    context.go(const HomeRoute().location);
  }
}
";

    assert!(typed_route_navigation_found(source)?);
    Ok(())
}

#[test]
fn unrelated_local_base_class_does_not_provide_state_context()
-> Result<(), Box<dyn std::error::Error>> {
    let source = r"
class BaseState<T> extends Object {}

class ScreenState extends BaseState<Screen> {
  void open() {
    context.go(const HomeRoute().location);
  }
}
";

    assert!(!typed_route_navigation_found(source)?);
    Ok(())
}

#[test]
fn state_context_resolves_aliased_framework_state() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
import 'package:flutter/widgets.dart' as f;

class ScreenState extends f.State<Screen> {
  void open() {
    context.go(const HomeRoute().location);
  }
}
";

    assert!(typed_route_navigation_found(source)?);
    Ok(())
}

#[test]
fn local_state_shadow_does_not_provide_state_context() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
class State<T> {}

class ScreenState extends State<Screen> {
  void open() {
    context.go(const HomeRoute().location);
  }
}
";

    assert!(!typed_route_navigation_found(source)?);
    Ok(())
}

#[test]
fn unresolved_prefixed_state_does_not_provide_state_context()
-> Result<(), Box<dyn std::error::Error>> {
    let source = r"
import 'package:not_flutter/fake.dart' as fake;

class ScreenState extends fake.State<Screen> {
  void open() {
    context.go(const HomeRoute().location);
  }
}
";

    assert!(!typed_route_navigation_found(source)?);
    Ok(())
}

#[test]
fn prefixed_state_does_not_fall_back_to_local_state_shadow()
-> Result<(), Box<dyn std::error::Error>> {
    let source = r"
import 'package:flutter/widgets.dart' as f;
import 'package:not_flutter/fake.dart' as fake;

class State<T> extends f.State<T> {}

class ScreenState extends fake.State<Screen> {
  void open() {
    context.go(const HomeRoute().location);
  }
}
";

    assert!(!typed_route_navigation_found(source)?);
    Ok(())
}

#[test]
fn route_location_named_argument_is_not_navigation_target() -> Result<(), Box<dyn std::error::Error>>
{
    let source = r"
class Screen {
  void open(BuildContext context) {
    context.go(location: const HomeRoute().location);
  }
}
";

    assert!(!typed_route_navigation_found(source)?);
    Ok(())
}
