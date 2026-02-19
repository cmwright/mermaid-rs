use criterion::{black_box, criterion_group, criterion_main, Criterion};
use mermaid_core::diagram::{render, OutputFormat, RenderConfig};
use mermaid_core::font::FontProvider;
use mermaid_core::layout::gantt::layout_gantt;
use mermaid_core::layout::text_measure::TextMeasurer;
use mermaid_core::parser::gantt::parse_gantt;
use mermaid_core::render::svg_gantt;
use mermaid_core::render::theme::Theme;

const SIMPLE: &str = r#"gantt
    title Project Timeline
    dateFormat YYYY-MM-DD
    section Phase 1
    Plan        :a1, 2025-01-01, 3d
    Build       :after a1, 5d
    Milestone   :milestone, m1, 2025-01-10, 0d
"#;

const MEDIUM: &str = r#"gantt
    title Product Launch Plan
    dateFormat YYYY-MM-DD
    axisFormat %b %d
    excludes weekends
    section Discovery
    Research          :a1, 2025-01-01, 5d
    Requirements      :after a1, 4d
    section Build
    Backend API       :b1, 2025-01-12, 10d
    Frontend UI       :b2, 2025-01-14, 9d
    Integration       :after b1 b2, 5d
    section Validation
    QA Cycle          :c1, after b1, 6d
    Security Review   :crit, c2, after c1, 3d
    Launch            :milestone, launch, after c2, 0d
"#;

const COMPLEX: &str = r#"gantt
    title Multi-team Platform Delivery
    dateFormat YYYY-MM-DD
    axisFormat %Y-%m-%d
    excludes weekends, sunday
    includes saturday
    tickInterval 1week
    section Foundations
    Infra Baseline            :f1, 2025-01-01, 14d
    Identity Setup            :f2, after f1, 8d
    Network Hardening         :crit, f3, after f1, 10d
    Shared Runtime            :f4, after f2 f3, 6d
    section Product Services
    Service A API             :s1, 2025-01-10, 18d
    Service B API             :s2, 2025-01-12, 20d
    Service C API             :s3, 2025-01-15, 16d
    Event Pipeline            :s4, after s1 s2 s3, 12d
    Data Migration            :s5, after s4, 7d
    section Experience
    Shell App                 :u1, 2025-01-18, 15d
    Feature Flags             :u2, after u1, 6d
    Analytics Instrumentation :u3, after u1, 8d
    Accessibility Pass        :u4, after u2 u3, 5d
    section Readiness
    End-to-end Tests          :r1, after s5 u4, 10d
    Load Test                 :active, r2, after r1, 4d
    Compliance Review         :crit, done, r3, after r1, 5d
    Go/No-Go                  :milestone, r4, after r2 r3, 0d
"#;

fn bench_end_to_end(c: &mut Criterion) {
    let config = RenderConfig {
        theme: Theme::default(),
        font_provider: FontProvider::default_font(),
        output_format: OutputFormat::Svg,
        width: None,
        background: None,
    };

    let mut group = c.benchmark_group("gantt_end_to_end");
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
    let mut group = c.benchmark_group("gantt_parse");
    group.bench_function("simple", |b| {
        b.iter(|| parse_gantt(black_box(SIMPLE)).unwrap())
    });
    group.bench_function("medium", |b| {
        b.iter(|| parse_gantt(black_box(MEDIUM)).unwrap())
    });
    group.bench_function("complex", |b| {
        b.iter(|| parse_gantt(black_box(COMPLEX)).unwrap())
    });
    group.finish();
}

fn bench_layout(c: &mut Criterion) {
    let provider = FontProvider::default_font();
    let font_ref = provider.font_ref().unwrap();
    let theme = Theme::default();
    let measurer = TextMeasurer::new(font_ref, theme.font_size as f32);

    let simple_ast = parse_gantt(SIMPLE).unwrap();
    let medium_ast = parse_gantt(MEDIUM).unwrap();
    let complex_ast = parse_gantt(COMPLEX).unwrap();

    let mut group = c.benchmark_group("gantt_layout");
    group.bench_function("simple", |b| {
        b.iter(|| {
            layout_gantt(
                black_box(&simple_ast),
                black_box(&measurer),
                black_box(&theme),
            )
            .unwrap()
        })
    });
    group.bench_function("medium", |b| {
        b.iter(|| {
            layout_gantt(
                black_box(&medium_ast),
                black_box(&measurer),
                black_box(&theme),
            )
            .unwrap()
        })
    });
    group.bench_function("complex", |b| {
        b.iter(|| {
            layout_gantt(
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

    let simple_layout = layout_gantt(&parse_gantt(SIMPLE).unwrap(), &measurer, &theme).unwrap();
    let medium_layout = layout_gantt(&parse_gantt(MEDIUM).unwrap(), &measurer, &theme).unwrap();
    let complex_layout = layout_gantt(&parse_gantt(COMPLEX).unwrap(), &measurer, &theme).unwrap();

    let mut group = c.benchmark_group("gantt_render_svg");
    group.bench_function("simple", |b| {
        b.iter(|| svg_gantt::render_svg(black_box(&simple_layout), black_box(&theme)).unwrap())
    });
    group.bench_function("medium", |b| {
        b.iter(|| svg_gantt::render_svg(black_box(&medium_layout), black_box(&theme)).unwrap())
    });
    group.bench_function("complex", |b| {
        b.iter(|| svg_gantt::render_svg(black_box(&complex_layout), black_box(&theme)).unwrap())
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
