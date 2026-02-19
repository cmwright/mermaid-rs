use criterion::{black_box, criterion_group, criterion_main, Criterion};
use mermaid_core::diagram::{render, OutputFormat, RenderConfig};
use mermaid_core::font::FontProvider;
use mermaid_core::layout::flowchart::layout_flowchart;
use mermaid_core::layout::text_measure::TextMeasurer;
use mermaid_core::parser::flowchart::parse_flowchart;
use mermaid_core::render::svg_flowchart;
use mermaid_core::render::theme::Theme;

// ─── Test Inputs ────────────────────────────────────────────

const SIMPLE: &str = r#"flowchart TD
    A[Start] --> B{Decision}
    B -->|Yes| C[OK]
    B -->|No| D[Fail]
    C --> E[End]
    D --> E
"#;

const MEDIUM: &str = r#"flowchart TD
    A[User Request] --> B{Authenticated?}
    B -->|Yes| C[Load Dashboard]
    B -->|No| D[Login Page]
    D --> E[Enter Credentials]
    E --> F{Valid?}
    F -->|Yes| C
    F -->|No| G[Show Error]
    G --> D
    C --> H[Fetch Data]
    H --> I[Transform]
    I --> J[Render View]
    J --> K{More Actions?}
    K -->|Yes| L[Process Action]
    L --> H
    K -->|No| M[End Session]
    H --> N[Cache Layer]
    N --> O[Database]
    O --> P[Return Results]
    P --> I
"#;

const COMPLEX: &str = r#"graph TD
    subgraph Platform
        subgraph OrgPkg["Organization: Lorem Corp"]
            RootOU["Root OU: Lorem US"]
            EUOU["Child OU: Lorem EU"]
            APACOU["Child OU: Lorem APAC"]
            RootOU -->|HAS_CHILD_OU| EUOU
            RootOU -->|HAS_CHILD_OU| APACOU
        end

        subgraph OrgPkg2["Organization: Ipsum Inc"]
            SmallOU["Root OU: Ipsum Inc"]
        end

        subgraph LEPkg1["Legal Entity: Lorem Corp Holdings"]
            LE1["Lorem Corp Holdings"]
            D1["lorem.com"]
            LE1 -->|OWNS_DOMAIN| D1
        end

        subgraph LEPkg2["Legal Entity: Lorem EU GmbH"]
            LE2["Lorem EU GmbH"]
            D2["lorem.eu"]
            LE2 -->|OWNS_DOMAIN| D2
        end

        subgraph LEPkg3["Legal Entity: Ipsum LLC"]
            LE3["Ipsum LLC"]
            D3["ipsum.io"]
            LE3 -->|OWNS_DOMAIN| D3
        end

        RootOU -.->|REPRESENTS| LE1
        EUOU -.->|REPRESENTS| LE2
        SmallOU -.->|REPRESENTS| LE3
    end

    subgraph OryNetwork["Identity Platform"]
        subgraph OryUS["Org: Lorem US"]
            OO1["Organization org-aaa"]
            SSO1["SAML Connection"]
            ID1["Identity: user1"]
            ID2["Identity: user2"]
            OO1 -->|SSO provider| SSO1
            OO1 -->|member| ID1
            OO1 -->|member| ID2
        end

        subgraph OryEU["Org: Lorem EU"]
            OO2["Organization org-bbb"]
            SSO2["OIDC Connection"]
            ID3["Identity: user3"]
            OO2 -->|SSO provider| SSO2
            OO2 -->|member| ID3
        end

        subgraph OrySmall["Org: Ipsum Inc"]
            OO3["Organization org-ccc"]
            ID4["Identity: user4"]
            ID5["Identity: user5"]
            OO3 -->|member| ID4
            OO3 -->|member| ID5
        end
    end

    subgraph ExtIdPs["External Identity Providers"]
        OktaIdP["Provider A IdP"]
        AzureIdP["Provider B IdP"]
    end

    RootOU ==>|org_id| OO1
    EUOU ==>|org_id| OO2
    APACOU -.->|inherits from parent| OO1
    SmallOU ==>|org_id| OO3

    D1 -->|configures domain| OO1
    D2 -->|configures domain| OO2
    D3 -->|configures domain| OO3

    SSO1 -->|SAML AuthnRequest| OktaIdP
    SSO2 -->|OIDC authorize| AzureIdP

    style OryUS fill:#d4eaff,stroke:#336
    style OryEU fill:#d4f5d4,stroke:#363
    style OrySmall fill:#f5e6ff,stroke:#636
    style APACOU fill:#fff3cd,stroke:#996
    style OktaIdP fill:#f5f5f5,stroke:#999
    style AzureIdP fill:#f5f5f5,stroke:#999
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

    let mut group = c.benchmark_group("end_to_end");

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
    let mut group = c.benchmark_group("parse");

    group.bench_function("simple", |b| {
        b.iter(|| parse_flowchart(black_box(SIMPLE)).unwrap())
    });

    group.bench_function("medium", |b| {
        b.iter(|| parse_flowchart(black_box(MEDIUM)).unwrap())
    });

    group.bench_function("complex", |b| {
        b.iter(|| parse_flowchart(black_box(COMPLEX)).unwrap())
    });

    group.finish();
}

fn bench_layout(c: &mut Criterion) {
    let provider = FontProvider::default_font();
    let font_ref = provider.font_ref().unwrap();
    let measurer = TextMeasurer::new(font_ref, 14.0);

    let simple_ast = parse_flowchart(SIMPLE).unwrap();
    let medium_ast = parse_flowchart(MEDIUM).unwrap();
    let complex_ast = parse_flowchart(COMPLEX).unwrap();

    let mut group = c.benchmark_group("layout");

    group.bench_function("simple", |b| {
        b.iter(|| layout_flowchart(black_box(&simple_ast), black_box(&measurer)).unwrap())
    });

    group.bench_function("medium", |b| {
        b.iter(|| layout_flowchart(black_box(&medium_ast), black_box(&measurer)).unwrap())
    });

    group.bench_function("complex", |b| {
        b.iter(|| layout_flowchart(black_box(&complex_ast), black_box(&measurer)).unwrap())
    });

    group.finish();
}

fn bench_render_svg(c: &mut Criterion) {
    let provider = FontProvider::default_font();
    let font_ref = provider.font_ref().unwrap();
    let measurer = TextMeasurer::new(font_ref, 14.0);
    let theme = Theme::default();

    let simple_ast = parse_flowchart(SIMPLE).unwrap();
    let medium_ast = parse_flowchart(MEDIUM).unwrap();
    let complex_ast = parse_flowchart(COMPLEX).unwrap();

    let simple_layout = layout_flowchart(&simple_ast, &measurer).unwrap();
    let medium_layout = layout_flowchart(&medium_ast, &measurer).unwrap();
    let complex_layout = layout_flowchart(&complex_ast, &measurer).unwrap();

    let mut group = c.benchmark_group("render_svg");

    group.bench_function("simple", |b| {
        b.iter(|| svg_flowchart::render_svg(black_box(&simple_layout), black_box(&theme)).unwrap())
    });

    group.bench_function("medium", |b| {
        b.iter(|| svg_flowchart::render_svg(black_box(&medium_layout), black_box(&theme)).unwrap())
    });

    group.bench_function("complex", |b| {
        b.iter(|| svg_flowchart::render_svg(black_box(&complex_layout), black_box(&theme)).unwrap())
    });

    group.finish();
}

fn bench_font_provider(c: &mut Criterion) {
    let mut group = c.benchmark_group("font");

    group.bench_function("default_font_create", |b| {
        b.iter(|| FontProvider::default_font())
    });

    let provider = FontProvider::default_font();
    group.bench_function("font_ref", |b| b.iter(|| provider.font_ref().unwrap()));

    group.finish();
}

criterion_group!(
    benches,
    bench_end_to_end,
    bench_parse,
    bench_layout,
    bench_render_svg,
    bench_font_provider,
);
criterion_main!(benches);
