use criterion::{black_box, criterion_group, criterion_main, Criterion};
use mermaid_core::diagram::{render, OutputFormat, RenderConfig};
use mermaid_core::font::FontProvider;
use mermaid_core::layout::sequence::layout_sequence;
use mermaid_core::layout::text_measure::TextMeasurer;
use mermaid_core::parser::sequence::parse_sequence;
use mermaid_core::render::svg_sequence;
use mermaid_core::render::theme::Theme;

// ─── Test Inputs ────────────────────────────────────────────

const SIMPLE: &str = r#"sequenceDiagram
    Alice->>Bob: Hello Bob
    Bob-->>Alice: Hi Alice
"#;

const MEDIUM: &str = r#"sequenceDiagram
    participant Client
    participant API as API Server
    participant Auth as Auth Service
    participant DB as Database

    Client->>API: POST /login
    API->>Auth: Validate credentials
    Auth->>DB: SELECT user
    DB-->>Auth: User record
    Auth-->>API: JWT token
    API-->>Client: 200 OK + token

    Client->>API: GET /data (Bearer token)
    API->>Auth: Verify token
    Auth-->>API: Valid
    API->>DB: SELECT data
    DB-->>API: Results
    API-->>Client: 200 OK + data
"#;

const COMPLEX: &str = r#"sequenceDiagram
    actor User
    participant UI as Factor UI
    participant Kratos as Ory Kratos
    participant IdP as External IdP
    participant SvcUsers as svc-users-v2
    participant DB as User Service DB

    User->>UI: Enter email
    UI->>Kratos: POST /self-service/login
    Kratos->>Kratos: Resolve domain

    alt SSO configured
        Kratos->>IdP: SAML AuthnRequest
        IdP->>User: Authentication prompt
        User->>IdP: Authenticate
        IdP->>Kratos: SAML Response
        Kratos->>Kratos: Create identity and issue session
    else Password only
        Kratos->>UI: Show password prompt
        User->>UI: Enter password
        UI->>Kratos: Submit credentials
        Kratos->>Kratos: Validate and issue session
    end

    Kratos->>SvcUsers: Webhook after login
    SvcUsers->>DB: Upsert user record
    UI->>SvcUsers: POST /v1/auth/token
    SvcUsers->>Kratos: GET /sessions/whoami
    Kratos-->>SvcUsers: Session and identity
    SvcUsers->>DB: Load roles and entitlements
    SvcUsers-->>UI: Self-minted JWT

    Note right of UI: Token cached in browser
    Note over Kratos,SvcUsers: Internal service mesh
"#;

// ─── Benchmarks ─────────────────────────────────────────────

fn bench_end_to_end(c: &mut Criterion) {
    let config = RenderConfig {
        theme: Theme::default(),
        font_provider: FontProvider::default_font(),
        output_format: OutputFormat::Svg,
        width: None,
        background: None,
    };

    let mut group = c.benchmark_group("seq_end_to_end");

    group.bench_function("simple", |b| {
        b.iter(|| render(black_box(SIMPLE), black_box(&config)).unwrap())
    });

    group.bench_function("medium", |b| {
        b.iter(|| render(black_box(MEDIUM), black_box(&config)).unwrap())
    });

    group.bench_function("complex", |b| {
        b.iter(|| render(black_box(COMPLEX), black_box(&config)).unwrap())
    });

    group.finish();
}

fn bench_parse(c: &mut Criterion) {
    let mut group = c.benchmark_group("seq_parse");

    group.bench_function("simple", |b| {
        b.iter(|| parse_sequence(black_box(SIMPLE)).unwrap())
    });

    group.bench_function("medium", |b| {
        b.iter(|| parse_sequence(black_box(MEDIUM)).unwrap())
    });

    group.bench_function("complex", |b| {
        b.iter(|| parse_sequence(black_box(COMPLEX)).unwrap())
    });

    group.finish();
}

fn bench_layout(c: &mut Criterion) {
    let provider = FontProvider::default_font();
    let font_ref = provider.font_ref().unwrap();
    let theme = Theme::default();
    let measurer = TextMeasurer::new(font_ref, theme.font_size as f32);

    let simple_ast = parse_sequence(SIMPLE).unwrap();
    let medium_ast = parse_sequence(MEDIUM).unwrap();
    let complex_ast = parse_sequence(COMPLEX).unwrap();

    let mut group = c.benchmark_group("seq_layout");

    group.bench_function("simple", |b| {
        b.iter(|| {
            layout_sequence(
                black_box(&simple_ast),
                black_box(&measurer),
                black_box(&theme),
            )
            .unwrap()
        })
    });

    group.bench_function("medium", |b| {
        b.iter(|| {
            layout_sequence(
                black_box(&medium_ast),
                black_box(&measurer),
                black_box(&theme),
            )
            .unwrap()
        })
    });

    group.bench_function("complex", |b| {
        b.iter(|| {
            layout_sequence(
                black_box(&complex_ast),
                black_box(&measurer),
                black_box(&theme),
            )
            .unwrap()
        })
    });

    group.finish();
}

fn bench_render_svg(c: &mut Criterion) {
    let provider = FontProvider::default_font();
    let font_ref = provider.font_ref().unwrap();
    let theme = Theme::default();
    let measurer = TextMeasurer::new(font_ref, theme.font_size as f32);

    let simple_ast = parse_sequence(SIMPLE).unwrap();
    let medium_ast = parse_sequence(MEDIUM).unwrap();
    let complex_ast = parse_sequence(COMPLEX).unwrap();

    let simple_layout = layout_sequence(&simple_ast, &measurer, &theme).unwrap();
    let medium_layout = layout_sequence(&medium_ast, &measurer, &theme).unwrap();
    let complex_layout = layout_sequence(&complex_ast, &measurer, &theme).unwrap();

    let mut group = c.benchmark_group("seq_render_svg");

    group.bench_function("simple", |b| {
        b.iter(|| svg_sequence::render_svg(black_box(&simple_layout), black_box(&theme)).unwrap())
    });

    group.bench_function("medium", |b| {
        b.iter(|| svg_sequence::render_svg(black_box(&medium_layout), black_box(&theme)).unwrap())
    });

    group.bench_function("complex", |b| {
        b.iter(|| svg_sequence::render_svg(black_box(&complex_layout), black_box(&theme)).unwrap())
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_end_to_end,
    bench_parse,
    bench_layout,
    bench_render_svg,
);
criterion_main!(benches);
