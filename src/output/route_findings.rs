use std::collections::BTreeSet;
use std::path::Path;

use super::format::display_path;
use super::route_cycle::{MixedGoRouterUse, MixedGoRouterUseKind, mixed_go_router_uses};
use super::{Finding, FindingAction, FindingKind, Severity};
use crate::{RouteCollision, RouteCollisionKind, RouteCollisionReport, scan::ScannedProject};

pub(super) fn add_route_findings(
    project: &ScannedProject,
    report: &RouteCollisionReport,
    findings: &mut Vec<Finding>,
) {
    findings.extend(
        report
            .collisions
            .iter()
            .filter_map(|collision| route_collision_finding(&project.root, collision)),
    );
    findings.extend(
        mixed_go_router_uses(project)
            .iter()
            .map(|usage| mixed_go_router_finding(&project.root, usage)),
    );
}

fn route_collision_finding(root: &Path, collision: &RouteCollision) -> Option<Finding> {
    let primary = collision
        .declarations
        .get(1)
        .or_else(|| collision.declarations.first())?;
    let path = display_path(root, &primary.path);
    let files = collision
        .declarations
        .iter()
        .map(|declaration| display_path(root, &declaration.path))
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let label = match collision.kind {
        RouteCollisionKind::Path => "path",
        RouteCollisionKind::Name => "name",
    };

    Some(Finding {
        rule_id: "dart-decimate/route-collision".to_owned(),
        fingerprint: Some(route_collision_fingerprint(collision)),
        kind: FindingKind::RouteCollision,
        severity: Severity::Error,
        message: format!(
            "GoRouter route {label} {} is declared by {} routes",
            collision.value,
            collision.declarations.len()
        ),
        path: path.clone(),
        line: primary.location.line,
        column: primary.location.column,
        safe_to_delete: false,
        files,
        edge: None,
        actions: vec![
            FindingAction::new(
                "review-route-collision",
                "Rename or move one route so GoRouter paths remain unique",
                false,
            )
            .with_target_path(path.clone())
            .with_target_symbol(primary.route_class.clone())
            .with_dart_decimate_args(["inspect", "--format", "json", "--file", path.as_str()])
            .with_suppression_comment("// dart-decimate-ignore-next-line route-collision"),
        ],
    })
}

fn mixed_go_router_finding(root: &Path, usage: &MixedGoRouterUse) -> Finding {
    let path = display_path(root, &usage.path);
    let (message, description, target_symbol) = match &usage.kind {
        MixedGoRouterUseKind::RouteDefinition => (
            "A raw `GoRoute` definition is used after typed routes were adopted".to_owned(),
            "Define the route with `TypedGoRoute` and `GoRouteData` so route construction remains compile-time checked",
            "GoRoute".to_owned(),
        ),
        MixedGoRouterUseKind::Navigation { method } => (
            format!("Raw GoRouter navigation `{method}` is used after typed routes were adopted"),
            "Navigate with the generated route object so route parameters remain compile-time checked",
            method.clone(),
        ),
        MixedGoRouterUseKind::RedirectDestination => (
            "A raw redirect destination is used after typed routes were adopted".to_owned(),
            "Return a generated typed route object's `.location` so redirect parameters remain compile-time checked",
            "redirect".to_owned(),
        ),
    };
    Finding {
        rule_id: "dart-decimate/mixed-go-router-style".to_owned(),
        fingerprint: Some(mixed_go_router_fingerprint(&path, usage)),
        kind: FindingKind::MixedGoRouterStyle,
        severity: Severity::Error,
        message,
        path: path.clone(),
        line: usage.location.line,
        column: usage.location.column,
        safe_to_delete: false,
        files: vec![path.clone()],
        edge: None,
        actions: vec![
            FindingAction::new("use-typed-go-router", description, false)
                .with_target_path(path)
                .with_target_symbol(target_symbol)
                .with_suppression_comment(
                    "// dart-decimate-ignore-next-line mixed-go-router-style -- <reason>",
                ),
        ],
    }
}

fn mixed_go_router_fingerprint(path: &str, usage: &MixedGoRouterUse) -> String {
    let mut hash = 0xcbf2_9ce4_8422_2325_u64;
    for byte in format!(
        "{}:{}:{}:{}",
        path,
        usage.location.line,
        usage.location.column,
        match &usage.kind {
            MixedGoRouterUseKind::RouteDefinition => "GoRoute",
            MixedGoRouterUseKind::Navigation { method } => method,
            MixedGoRouterUseKind::RedirectDestination => "redirect",
        }
    )
    .as_bytes()
    {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    format!("route-style:{:08x}", hash & 0xffff_ffff)
}

fn route_collision_fingerprint(collision: &RouteCollision) -> String {
    let mut hash = 0xcbf2_9ce4_8422_2325_u64;
    for byte in format!("{:?}:{}", collision.kind, collision.value).as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    format!("route:{:08x}", hash & 0xffff_ffff)
}
