use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::generated::is_generated_dart_path;
use crate::{DartRouteDeclaration, Location, scan::ScannedProject};

/// `GoRouter` route collision analysis.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RouteCollisionReport {
    /// Duplicate route paths or names.
    pub collisions: Vec<RouteCollision>,
}

/// A route path or route name declared by more than one typed route.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RouteCollision {
    /// Collision category.
    pub kind: RouteCollisionKind,
    /// Human-readable conflicting path or name.
    pub value: String,
    /// Declarations that share this route identity.
    pub declarations: Vec<RouteCollisionDeclaration>,
}

/// Collision category.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum RouteCollisionKind {
    /// Two or more routes resolve to the same path pattern.
    Path,
    /// Two or more routes use the same route name.
    Name,
}

/// One route declaration participating in a collision.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RouteCollisionDeclaration {
    /// Dart file containing the route declaration.
    pub path: PathBuf,
    /// Typed route data class.
    pub route_class: String,
    /// Location of the annotation or route constructor.
    pub location: Location,
}

/// Detect duplicate `GoRouter` paths and names.
#[must_use]
pub fn detect_route_collisions(project: &ScannedProject) -> RouteCollisionReport {
    let mut path_groups = BTreeMap::<String, CollisionGroup>::new();
    let mut name_groups = BTreeMap::<String, CollisionGroup>::new();

    for file in &project.files {
        if is_generated_dart_path(&file.path) || is_test_dart(&file.path) {
            continue;
        }
        for route in &file.routes {
            let declaration = collision_declaration(&file.path, route);
            let scope = route_collision_scope(&file.path, route);
            if let Some(path) = &route.path {
                let key = scoped_collision_key(scope.as_deref(), &route_path_key(path));
                path_groups
                    .entry(key)
                    .or_insert_with(|| CollisionGroup::new(path.clone()))
                    .declarations
                    .push(declaration.clone());
            }
            if let Some(name) = &route.name {
                let key = scoped_collision_key(scope.as_deref(), name);
                name_groups
                    .entry(key)
                    .or_insert_with(|| CollisionGroup::new(name.clone()))
                    .declarations
                    .push(declaration);
            }
        }
    }

    RouteCollisionReport {
        collisions: collisions(RouteCollisionKind::Path, path_groups)
            .into_iter()
            .chain(collisions(RouteCollisionKind::Name, name_groups))
            .collect(),
    }
}

#[derive(Debug, Clone)]
struct CollisionGroup {
    value: String,
    declarations: Vec<RouteCollisionDeclaration>,
}

impl CollisionGroup {
    fn new(value: String) -> Self {
        Self {
            value,
            declarations: Vec::new(),
        }
    }
}

fn collisions(
    kind: RouteCollisionKind,
    groups: BTreeMap<String, CollisionGroup>,
) -> Vec<RouteCollision> {
    groups
        .into_values()
        .filter_map(|mut group| {
            group.declarations.sort_by(|left, right| {
                (
                    &left.path,
                    left.location.line,
                    left.location.column,
                    &left.route_class,
                )
                    .cmp(&(
                        &right.path,
                        right.location.line,
                        right.location.column,
                        &right.route_class,
                    ))
            });
            (group.declarations.len() > 1).then_some(RouteCollision {
                kind,
                value: group.value,
                declarations: group.declarations,
            })
        })
        .collect()
}

fn collision_declaration(path: &Path, route: &DartRouteDeclaration) -> RouteCollisionDeclaration {
    RouteCollisionDeclaration {
        path: path.to_path_buf(),
        route_class: route.route_class.clone(),
        location: route.location,
    }
}

fn route_collision_scope(path: &Path, route: &DartRouteDeclaration) -> Option<String> {
    (route.route_class == "GoRoute").then(|| path.to_string_lossy().into_owned())
}

fn scoped_collision_key(scope: Option<&str>, value: &str) -> String {
    match scope {
        Some(scope) => format!("{scope}\0{value}"),
        None => value.to_owned(),
    }
}

fn route_path_key(path: &str) -> String {
    normalize_route_path(path)
        .split('/')
        .map(|segment| {
            if segment.starts_with(':') {
                ":"
            } else {
                segment
            }
        })
        .collect::<Vec<_>>()
        .join("/")
}

fn normalize_route_path(path: &str) -> String {
    let mut normalized = path.replace("//", "/");
    while normalized.len() > 1 && normalized.ends_with('/') {
        normalized.pop();
    }
    if normalized.is_empty() {
        "/".to_owned()
    } else if normalized.starts_with('/') {
        normalized
    } else {
        format!("/{normalized}")
    }
}

fn is_test_dart(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name.ends_with("_test.dart"))
        || path.components().any(|component| {
            component.as_os_str().to_str().is_some_and(|segment| {
                matches!(segment, "test" | "integration_test" | "test_driver")
            })
        })
}

#[cfg(test)]
mod tests {
    use crate::extract_dart_source;

    use super::*;

    #[test]
    fn detects_param_name_route_collisions() -> Result<(), Box<dyn std::error::Error>> {
        let project = test_project(&[
            (
                "lib/a.dart",
                "@TypedGoRoute<UserRoute>(path: '/users/:id')\nclass UserRoute extends GoRouteData {}\n",
            ),
            (
                "lib/b.dart",
                "@TypedGoRoute<MemberRoute>(path: '/users/:userId')\nclass MemberRoute extends GoRouteData {}\n",
            ),
        ])?;

        let report = detect_route_collisions(&project);

        assert_eq!(report.collisions.len(), 1);
        assert_eq!(report.collisions[0].kind, RouteCollisionKind::Path);
        assert_eq!(report.collisions[0].declarations.len(), 2);
        Ok(())
    }

    #[test]
    fn does_not_collide_static_and_dynamic_siblings() -> Result<(), Box<dyn std::error::Error>> {
        let project = test_project(&[
            (
                "lib/a.dart",
                "@TypedGoRoute<NewUserRoute>(path: '/users/new')\nclass NewUserRoute extends GoRouteData {}\n",
            ),
            (
                "lib/b.dart",
                "@TypedGoRoute<UserRoute>(path: '/users/:id')\nclass UserRoute extends GoRouteData {}\n",
            ),
        ])?;

        let report = detect_route_collisions(&project);

        assert!(report.collisions.is_empty());
        Ok(())
    }

    #[test]
    fn does_not_collide_same_child_segment_under_different_parents()
    -> Result<(), Box<dyn std::error::Error>> {
        let source = "
@TypedGoRoute<UserRoute>(
  path: '/users',
  routes: [TypedGoRoute<UserDetailsRoute>(path: 'details')],
)
class UserRoute extends GoRouteData {}

@TypedGoRoute<OrderRoute>(
  path: '/orders',
  routes: [TypedGoRoute<OrderDetailsRoute>(path: 'details')],
)
class OrderRoute extends GoRouteData {}
";
        let project = test_project(&[("lib/routes.dart", source)])?;

        let report = detect_route_collisions(&project);

        assert!(report.collisions.is_empty());
        Ok(())
    }

    #[test]
    fn does_not_collide_raw_routes_from_independent_files() -> Result<(), Box<dyn std::error::Error>>
    {
        let project = test_project(&[
            (
                "example/lib/app_a.dart",
                "final router = GoRouter(routes: [GoRoute(path: '/', builder: (_, _) => const SizedBox())]);",
            ),
            (
                "example/lib/app_b.dart",
                "final router = GoRouter(routes: [GoRoute(path: '/', builder: (_, _) => const SizedBox())]);",
            ),
        ])?;

        let report = detect_route_collisions(&project);

        assert!(report.collisions.is_empty());
        Ok(())
    }

    #[test]
    fn still_collides_raw_routes_inside_one_route_file() -> Result<(), Box<dyn std::error::Error>> {
        let project = test_project(&[(
            "lib/router.dart",
            "
final router = GoRouter(routes: [
  GoRoute(path: '/', builder: (_, _) => const SizedBox()),
  GoRoute(path: '/', builder: (_, _) => const SizedBox()),
]);
",
        )])?;

        let report = detect_route_collisions(&project);

        assert_eq!(report.collisions.len(), 1);
        assert_eq!(report.collisions[0].kind, RouteCollisionKind::Path);
        Ok(())
    }

    #[test]
    fn skips_test_route_fixtures() -> Result<(), Box<dyn std::error::Error>> {
        let project = test_project(&[
            (
                "test/app_router_test.dart",
                "final routes = [GoRoute(path: '/settings', builder: (_, _) => const SizedBox())];",
            ),
            (
                "integration_test/app_flow_test.dart",
                "final routes = [GoRoute(path: '/settings', builder: (_, _) => const SizedBox())];",
            ),
        ])?;

        let report = detect_route_collisions(&project);

        assert!(report.collisions.is_empty());
        Ok(())
    }

    #[test]
    fn skips_generated_route_companions() -> Result<(), Box<dyn std::error::Error>> {
        let project = test_project(&[
            (
                "lib/routes.dart",
                "@TypedGoRoute<HomeRoute>(path: '/')\nclass HomeRoute extends GoRouteData {}\n",
            ),
            (
                "lib/routes.gen.dart",
                "@TypedGoRoute<GeneratedRoute>(path: '/')\nclass GeneratedRoute extends GoRouteData {}\n",
            ),
            (
                "lib/routes.mocks.dart",
                "@TypedGoRoute<MockRoute>(path: '/')\nclass MockRoute extends GoRouteData {}\n",
            ),
        ])?;

        let report = detect_route_collisions(&project);

        assert!(report.collisions.is_empty());
        Ok(())
    }

    fn test_project(
        sources: &[(&str, &str)],
    ) -> Result<ScannedProject, Box<dyn std::error::Error>> {
        let temp = tempfile::tempdir()?;
        let root = temp.path().to_path_buf();
        let files = sources
            .iter()
            .map(|(path, source)| extract_dart_source(root.join(path), source))
            .collect::<Result<Vec<_>, _>>()?;
        let graph = crate::build_module_graph(&root, &files)?;
        Ok(ScannedProject {
            scan_roots: vec![root.clone()],
            root,
            files,
            graph,
        })
    }
}
