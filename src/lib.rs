//! Dart Decimate analyzes Dart and Flutter repositories as module graphs.
//!
//! Phase 1 exposes a fast Tree-Sitter based extractor for imports, exports, and
//! top-level declarations. Phase 2 builds a directed module graph from those
//! syntax facts. Phase 3 runs graph traversals without evaluating variables,
//! types, or function bodies. Phase 4 exposes the CLI and JSON output contract.

pub mod baseline;
pub mod boundaries;
pub(crate) mod changed_scope;
pub mod ci_template;
pub mod cli;
pub mod config;
pub mod coverage;
pub(crate) mod dart_parser;
pub mod decision_surface;
pub mod dependencies;
pub(crate) mod dependency_scripts;
pub mod dupes;
pub mod explain;
pub mod extract;
pub mod feature_flags;
pub(crate) mod finding_identity;
pub mod fix;
pub mod flutter_style;
pub(crate) mod generated;
pub(crate) mod git_command;
pub mod graph;
pub mod health;
pub mod hooks;
pub mod impact;
pub mod init;
pub mod inspect;
pub mod intelligence;
pub mod manifest;
pub mod mcp;
pub mod output;
pub(crate) mod package_map;
pub mod policy;
pub mod project_list;
pub mod report_schema;
pub mod routes;
pub mod scan;
pub mod security;
pub(crate) mod security_gate;
pub mod semantic;
pub mod symbols;
pub mod trace;
pub mod unsupported;
pub mod widgets;
pub mod workspace_scope;

pub use baseline::{
    BASELINE_SCHEMA_VERSION, Baseline, BaselineError, BaselineFinding,
    REGRESSION_BASELINE_SCHEMA_VERSION, RegressionBaseline, RegressionComparison,
    RegressionCountDelta, RegressionCounts, RegressionTolerance, apply_baseline_to_report,
    baseline_from_report, compare_regression_baseline, load_baseline, load_regression_baseline,
    regression_baseline_from_report, save_baseline, save_regression_baseline,
};
pub use boundaries::{
    BoundaryAccessRule, BoundaryCoverageGap, BoundaryInventory, BoundaryPreset, BoundaryZone,
    boundary_inventory, boundary_preset_rules, detect_boundary_coverage,
};
pub use ci_template::{
    CI_TEMPLATE_SCHEMA_VERSION, CiTemplateError, CiTemplateFile, CiTemplatePlatform,
    CiTemplateReport, ci_template_report, render_ci_template, vendor_ci_template,
};
pub use config::{
    CONFIG_SCHEMA_VERSION, CacheConfig, ConfigError, ConfigOutputFormat, DartDecimateConfig,
    DuplicateConfig, FeatureFlagConfig, HealthConfig, IgnoreDependencyOverrideRule, SecurityConfig,
    config_schema, load_dart_decimate_config, missing_suppression_reasons_enabled,
};
pub use coverage::{
    COVERAGE_ANALYSIS_SCHEMA_VERSION, CoverageAnalysisReport, CoverageSetupConfigSummary,
    CoverageSetupFile, CoverageSetupFileAction, CoverageSetupReport, CoverageSetupSummary,
    CoverageUploadFile, CoverageUploadReport, CoverageUploadSummary, coverage_analysis_report,
    coverage_inventory_upload_report, coverage_setup_report, coverage_source_maps_upload_report,
    render_coverage_analysis_report, render_coverage_setup_report, render_coverage_upload_report,
};
pub use decision_surface::{
    DECISION_SURFACE_SCHEMA_VERSION, DecisionSurfaceCategory, DecisionSurfaceDecision,
    DecisionSurfaceReport, DecisionSurfaceSummary, decision_surface_report,
    render_decision_surface_report,
};
pub use dependencies::{
    DeclaredPackageDependency, DependencyHygieneError, DependencyHygieneReport, DependencyIssue,
    DependencyOverrideMisconfigReason, DependencySection, LocalPubPackage,
    MisconfiguredDependencyOverride, PrivateSrcImport, UnlistedPackageDependency,
    UnusedPackageDependency, analyze_dependency_hygiene, declared_package_dependencies,
    local_pub_packages,
};
pub use dupes::{
    CloneTraceReport, CodeClone, CodeCloneInstance, DuplicateCodeError, DuplicateCodeReport,
    DuplicateMode, DuplicateOptions, DuplicateStats, DuplicationThreshold, TraceCloneGroup,
    TraceCloneInstance, detect_duplicates, render_clone_trace, trace_clone,
};
pub use explain::{
    EXPLAIN_SCHEMA_VERSION, ExplainError, ExplainReport, explain_issue, render_explain_report,
};
pub use extract::{
    DartCombinator, DartCombinatorKind, DartExport, DartFile, DartImport, DartLibrary, DartPart,
    DartPartOf, DartRouteDeclaration, DartUriCondition, DeclarationKind, ExtractError,
    IdentifierReference, Location, MemberDeclaration, MemberKind, SignatureReference, SourceRange,
    TopLevelDeclaration, extract_dart_file, extract_dart_source,
};
pub use feature_flags::{
    FeatureFlag, FeatureFlagConfidence, FeatureFlagError, FeatureFlagOccurrence,
    FeatureFlagOptions, FeatureFlagReport, FeatureFlagSource, detect_feature_flags,
};
pub use fix::{
    FIX_SCHEMA_VERSION, FixChange, FixMode, FixReport, FixSkip, FixSummary, fix_findings,
    render_fix_report,
};
pub use flutter_style::{
    FlutterStyleFinding, FlutterStyleFindingKind, FlutterStyleReport, FlutterStyleValueKind,
    ThemeTokenEvidence, analyze_flutter_style,
};
pub use graph::{
    DependencyEdge, DependencyKind, DependencyVisibility, GraphError, GraphOptions,
    InvalidPartReason, InvalidPartRelationship, ModuleGraph, ModuleNode, ResolvedDependency,
    UnresolvedDependency, build_module_graph, build_module_graph_with_options,
};
pub use health::{
    ComplexityContribution, ComplexityFinding, ComplexityFunctionKind, ComplexityRule,
    CoverageGapFinding, CoverageGapReason, CrapFinding, EffectiveThresholds, FileCoverageStatus,
    FileHealthScore, HealthError, HealthHotspot, HealthOptions, HealthReport,
    HealthThresholdOverride, HealthThresholdOverrideReport, HealthThresholdOverrideStatus,
    HealthToggle, LargeFunction, LowTrafficThreshold, RefactoringTarget, RuntimeBlastRadius,
    RuntimeBlastRisk, RuntimeCoverageAction, RuntimeCoverageConfidence, RuntimeCoverageFinding,
    RuntimeCoverageFindingKind, RuntimeCoverageFormat, RuntimeCoverageIntelligence,
    RuntimeCoverageIntelligenceKind, RuntimeCoverageReport, RuntimeHotPath, RuntimeImportance,
    SourceMapConfidence, ThresholdSource, analyze_health,
};
pub use hooks::{
    HOOKS_SCHEMA_VERSION, HookAction, HookFile, HookOptions, HookTarget, HooksError, HooksReport,
    hooks_status, install_hooks, render_hooks_report, uninstall_hooks,
};
pub use impact::{
    IMPACT_SCHEMA_VERSION, ImpactAllReport, ImpactAllSummary, ImpactGate, ImpactProject,
    ImpactProjectSummary, ImpactRecord, ImpactReport, ImpactSort, ImpactTotals, ImpactTrend,
    impact_all_report, impact_report, render_impact_all_report, render_impact_report,
};
pub use init::{
    INIT_SCHEMA_VERSION, InitError, InitFile, InitFileAction, InitFileKind, InitOptions,
    InitReport, init_project, render_init_report,
};
pub use inspect::{
    INSPECT_SCHEMA_VERSION, InspectReport, InspectTarget, inspect_file, inspect_symbol,
    render_inspect_report,
};
pub use intelligence::{
    BoundaryRule, BoundaryViolation, DeadCodeReport, DeadFile, DependencyCycle, ReExportCycle,
    check_architecture_boundaries, detect_cycles, detect_re_export_cycles, find_dead_code,
};
pub use manifest::{MANIFEST_SCHEMA_VERSION, dart_decimate_schema};
pub use output::{
    AuditAttribution, AuditAttributionCounts, AuditRiskLevel, Finding, JsonAttackSurfaceEntry,
    JsonCloneGroup, JsonCloneInstance, JsonComplexityContribution, JsonComplexityFinding,
    JsonEffectiveThresholds, JsonFeatureFlag, JsonFeatureFlagOccurrence, JsonFileHealthScore,
    JsonHealthHotspot, JsonRefactoringTarget, JsonReport, JsonRuntimeBlastRadius,
    JsonRuntimeCoverage, JsonRuntimeCoverageActionable, JsonRuntimeCoverageFinding,
    JsonRuntimeCoverageIntelligence, JsonRuntimeCoverageProvenance, JsonRuntimeCoverageSummary,
    JsonRuntimeCoverageWatermark, JsonRuntimeHotPath, JsonRuntimeImportance, JsonSecurityCandidate,
    JsonSecurityOccurrence, JsonSecurityReachability, JsonThresholdOverride, ReportCommand,
    ReportSummary, TRACE_SCHEMA_VERSION, Verdict, build_json_report,
};
pub use policy::{
    BoundaryCallRule, BoundaryCallViolation, PolicyError, PolicyPack, PolicyRule, PolicyRuleKind,
    PolicySeverity, PolicyViolation, RULE_PACK_SCHEMA_VERSION, detect_boundary_call_violations,
    detect_policy_violations, load_policy_pack, rule_pack_schema,
};
pub use project_list::{
    ListedBoundaries, ListedBoundaryRule, ListedBoundaryZone, ListedEntryPoint, ListedFile,
    ListedPlugin, ListedWorkspace, PROJECT_LIST_SCHEMA_VERSION, ProjectListBoundaryConfig,
    ProjectListOptions, ProjectListReport, ProjectListSection, ProjectListSummary,
    project_list_report,
};
pub use report_schema::report_schema;
pub use routes::{
    RouteCollision, RouteCollisionDeclaration, RouteCollisionKind, RouteCollisionReport,
    detect_route_collisions,
};
pub use scan::{ScanError, ScanOptions, ScannedProject, scan_project, scan_project_with_options};
pub use security::{
    AttackSurfaceEntry, SecurityBlindSpot, SecurityBlindSpotReason, SecurityCandidate,
    SecurityCategory, SecurityConfidence, SecurityDefaultSeverity, SecurityError,
    SecurityOccurrence, SecurityOptions, SecurityReachability, SecurityReport,
    SecurityTaintConfidence, analyze_security,
};
pub use semantic::{
    SemanticCompleteness, SemanticDecision, SemanticEvidence, SemanticIdentity,
    SemanticOmissionReason, SemanticReport, SemanticTypeCoupling,
};
pub use symbols::{
    DuplicateExport, DuplicateExportDeclaration, PrivateTypeLeak, SymbolAnalysisOptions,
    SymbolIndex, SymbolReport, UnusedExport, UnusedMember, analyze_symbols,
    analyze_symbols_with_options, analyze_unused_exports,
};
pub use trace::{
    DependencyTraceReport, FileTraceReport, SymbolTraceReport, TraceDeclaration, TraceDependency,
    TraceDependencyDirective, TraceImpactPath, TracePubspecDependency, TraceReference,
    TraceSemanticCompleteness, TraceTestSuggestion, render_dependency_trace, render_file_trace,
    render_symbol_trace, trace_dependency, trace_file, trace_symbol,
};
pub use widgets::{
    MissingContextMountedAfterAwait, PrivateWidgetClass, UnrenderedWidgetClass, UnusedWidgetParam,
    WidgetAnalysisError, WidgetClassKind, WidgetReport, WidgetTopLevelFunction, analyze_widgets,
};
pub use workspace_scope::{
    WorkspaceScopeError, changed_workspace_file_scope, workspace_file_scope,
};
