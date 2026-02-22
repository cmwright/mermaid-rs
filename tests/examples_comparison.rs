//! Integration test that renders all examples from the Mermaid examples page
//! and generates an HTML comparison page showing input, mermaid-rs output, and mermaid.js output.

use mermaid_core::{render, RenderConfig};
use std::fs;

struct Example {
    name: &'static str,
    category: &'static str,
    source: &'static str,
}

const EXAMPLES: &[Example] = &[
    // ── Flowcharts ──────────────────────────────────────────
    Example {
        name: "Basic flowchart",
        category: "Flowchart",
        source: r#"graph LR
    A[Square Rect] -- Link text --> B((Circle))
    A --> C(Round Rect)
    B --> D{Rhombus}
    C --> D"#,
    },
    Example {
        name: "Larger flowchart with styling",
        category: "Flowchart",
        source: r#"graph TB
    sq[Square shape] --> ci((Circle shape))

    subgraph A
        od>Odd shape]-- Two line<br/>edge comment --> ro
        di{Diamond with <br/> line break} -.-> ro(Rounded<br>square<br>shape)
        di==>ro2(Rounded square shape)
    end

    e --> od3>Really long text with linebreak<br>in an Odd shape]

    e((Inner / circle<br>and some odd <br>special characters)) --> f(,.?!+-*ز)

    cyr[Cyrillic]-->cyr2((Circle shape Начало));

     classDef green fill:#9f6,stroke:#333,stroke-width:2px;
     classDef orange fill:#f96,stroke:#333,stroke-width:4px;
     class sq,e green
     class di orange"#,
    },
    Example {
        name: "LR Flowchart - Dependency diagram with dotted edges",
        category: "Flowchart",
        source: r#"flowchart LR
    USO[User Service Org data sync] --> FF[Feature Flagging]
    USO --> VM[Vendor Management]
    USO --> FVM[File Vault Management]
    USO --> OO[Organization Onboarding]

    FF --> RCT[Rebrand Colors Typography]
    RCT --> RGUN[Rebrand Global UI NAV]

    USO -. "blocked by" .-> HC[HC services live in Titan]
    HC --> IHCTUS[Integrate HC to Titan User Service]
    IHCTUS --> AUHCE[Assign users to HC entities]

    AUHCE -. "blocked by" .-> AOSI[Assess Org Service Integration]

    AOSI --> AQPF[AI Questionnaire Pre-Fill]
    AOSI --> AAW[Assessment Approval Workflow]
    AOSI --> VA[Vendor Assessments]
    AOSI --> IDOA[Include Docs and Obs in Assessments]
    AOSI --> SVQ[Send a vendor a questionnaire]

    AAW --> IRQ[Inherent Risk Questionnaire]
    IRQ --> SNI[ServiceNow Integration]
    AAW --> CQ[Conditional Questions]
    CQ --> ATI[AI Template Import]
    ATI --> ATG[AI Template Generation]
    AAW --> FRT[Findings and Risk Treatments]
    AAW --> ASR[Assessment Summary Report]
    ASR --> APLR[Assessment Program Level Reporting]
    APLR --> AR[Automated Re-assessment]
    AAW --> RA[Reviewer Assignments]

    VA --> ATRA[AI Text Response Analysis]
    ATRA --> STA[Scoring Text Answers]
    VA --> LAVM[Launch Assessments from Vendor Manager]

    VM -. "also blocked by" .-> IDOA
    IDOA --> FI[Filevault Integration]
    IDOA --> AEA[AI Evidence Analysis]

    OO -. "blocked by" .-> SVQ
    FVM -. "blocked by" .-> SVQ
    SVQ --> SRS[Scheduled Recurring Send]

    AUHCE -. "blocked by" .-> RSQ[Respond - Streamlined Questionnaire]
    AUHCE -. "blocked by" .-> RNTQ[Respond to a non-Titan questionnaire via file upload]
    RNTQ --> RSQ
    OO -. "blocked by" .-> RTQ[Respond to a Titan questionnaire]
    RTQ --> DL[Data Localization]"#,
    },
    Example {
        name: "TD Flowchart - Dependency diagram with dotted edges",
        category: "Flowchart",
        source: r#"flowchart TD
    USO[User Service Org data sync] --> FF[Feature Flagging]
    USO --> VM[Vendor Management]
    USO --> FVM[File Vault Management]
    USO --> OO[Organization Onboarding]

    FF --> RCT[Rebrand Colors Typography]
    RCT --> RGUN[Rebrand Global UI NAV]

    USO -. "blocked by" .-> HC[HC services live in Titan]
    HC --> IHCTUS[Integrate HC to Titan User Service]
    IHCTUS --> AUHCE[Assign users to HC entities]

    AUHCE -. "blocked by" .-> AOSI[Assess Org Service Integration]

    AOSI --> AQPF[AI Questionnaire Pre-Fill]
    AOSI --> AAW[Assessment Approval Workflow]
    AOSI --> VA[Vendor Assessments]
    AOSI --> IDOA[Include Docs and Obs in Assessments]
    AOSI --> SVQ[Send a vendor a questionnaire]

    AAW --> IRQ[Inherent Risk Questionnaire]
    IRQ --> SNI[ServiceNow Integration]
    AAW --> CQ[Conditional Questions]
    CQ --> ATI[AI Template Import]
    ATI --> ATG[AI Template Generation]
    AAW --> FRT[Findings and Risk Treatments]
    AAW --> ASR[Assessment Summary Report]
    ASR --> APLR[Assessment Program Level Reporting]
    APLR --> AR[Automated Re-assessment]
    AAW --> RA[Reviewer Assignments]

    VA --> ATRA[AI Text Response Analysis]
    ATRA --> STA[Scoring Text Answers]
    VA --> LAVM[Launch Assessments from Vendor Manager]

    VM -. "also blocked by" .-> IDOA
    IDOA --> FI[Filevault Integration]
    IDOA --> AEA[AI Evidence Analysis]

    OO -. "blocked by" .-> SVQ
    FVM -. "blocked by" .-> SVQ
    SVQ --> SRS[Scheduled Recurring Send]

    AUHCE -. "blocked by" .-> RSQ[Respond - Streamlined Questionnaire]
    AUHCE -. "blocked by" .-> RNTQ[Respond to a non-Titan questionnaire via file upload]
    RNTQ --> RSQ
    OO -. "blocked by" .-> RTQ[Respond to a Titan questionnaire]
    RTQ --> DL[Data Localization]"#,
    },
    Example {
        name: "Complex Organization Flowchart",
        category: "Flowchart",
        source: include_str!("../tests/test_loop/input_mermaid.mmd"),
    },
    Example {
        name: "Multi Subgraph Flowchart",
        category: "Flowchart",
        source: include_str!("../tests/test_loop/test_graphs.mmd"),
    },
    Example {
        name: "Complex Nested Subgraphs (CI/CD + Environments)",
        category: "Flowchart",
        source: include_str!("../tests/test_loop/complex_subgraphs.mmd"),
    },
    // ── Sequence Diagrams ───────────────────────────────────
    Example {
        name: "Basic sequence diagram",
        category: "Sequence Diagram",
        source: r#"sequenceDiagram
    Alice ->> Bob: Hello Bob, how are you?
    Bob-->>John: How about you John?
    Bob--x Alice: I am good thanks!
    Bob-x John: I am good thanks!
    Note right of John: Bob thinks a long<br/>long time, so long<br/>that the text does<br/>not fit on a row.

    Bob-->Alice: Checking with John...
    Alice->John: Yes... John, how are you?"#,
    },
    Example {
        name: "Loops, alt and opt",
        category: "Sequence Diagram",
        source: r#"sequenceDiagram
    loop Daily query
        Alice->>Bob: Hello Bob, how are you?
        alt is sick
            Bob->>Alice: Not so good :(
        else is well
            Bob->>Alice: Feeling fresh like a daisy
        end

        opt Extra response
            Bob->>Alice: Thanks for asking
        end
    end"#,
    },
    Example {
        name: "Message to self in loop",
        category: "Sequence Diagram",
        source: r#"sequenceDiagram
    participant Alice
    participant Bob
    Alice->>John: Hello John, how are you?
    loop HealthCheck
        John->>John: Fight against hypochondria
    end
    Note right of John: Rational thoughts<br/>prevail...
    John-->>Alice: Great!
    John->>Bob: How about you?
    Bob-->>John: Jolly good!"#,
    },
    Example {
        name: "Blogging app service communication",
        category: "Sequence Diagram",
        source: r#"sequenceDiagram
    participant web as Web Browser
    participant blog as Blog Service
    participant account as Account Service
    participant mail as Mail Service
    participant db as Storage

    Note over web,db: The user must be logged in to submit blog posts
    web->>+account: Logs in using credentials
    account->>db: Query stored accounts
    db->>account: Respond with query result

    alt Credentials not found
        account->>web: Invalid credentials
    else Credentials found
        account->>-web: Successfully logged in

        Note over web,db: When the user is authenticated, they can now submit new posts
        web->>+blog: Submit new post
        blog->>db: Store post data

        par Notifications
            blog--)mail: Send mail to blog subscribers
            blog--)db: Store in-site notifications
        and Response
            blog-->>-web: Successfully posted
        end
    end"#,
    },
    // ── Pie Charts ──────────────────────────────────────────
    Example {
        name: "Basic Pie Chart (Netflix)",
        category: "Pie Chart",
        source: r#"pie title NETFLIX
         "Time spent looking for movie" : 90
         "Time spent watching it" : 10"#,
    },
    Example {
        name: "Basic Pie Chart (Voldemort)",
        category: "Pie Chart",
        source: r#"pie title What Voldemort doesn't have?
         "FRIENDS" : 2
         "FAMILY" : 3
         "NOSE" : 45"#,
    },
    // ── Gantt Charts ────────────────────────────────────────
    Example {
        name: "Gantt chart",
        category: "Gantt Chart",
        source: r#"gantt
    title A Gantt Diagram
    dateFormat YYYY-MM-DD
    axisFormat %Y-%m-%d
    excludes weekends

    section Section A
    Completed task :done, des1, 2014-01-06, 2014-01-08
    Active task :active, des2, 2014-01-09, 3d
    Future task : des3, after des2, 5d
    Future task2 : des4, after des3, 5d

    section Critical tasks
    Completed critical task :crit, done, 2014-01-06, 24h
    Important milestone :crit, milestone, 2014-01-12, 0d"#,
    },
    Example {
        name: "Gantt chart - complex dependency handling",
        category: "Gantt Chart",
        source: r#"gantt
    title Complex Dependency Gantt (Readable)
    dateFormat YYYY-MM-DD
    axisFormat %Y-%m-%d
    excludes weekends

    section Discovery
    Kickoff :kickoff, 2026-03-02, 1d
    Requirements :req, after kickoff, 4d
    Risk review :risk, after kickoff, 2d
    Scope freeze :milestone, scope, after req, 0d, dependsOn req

    section Platform
    Infra setup :infra, 2026-03-03, 6d, dependsOn kickoff
    Auth service :auth, after infra, 5d, dependsOn infra
    Data contracts :contracts, after req, 3d, dependsOn req
    Integration gate :milestone, gate1, after auth, 0d, dependsOn auth contracts

    section Product
    UI implementation :ui, after scope, 6d, dependsOn scope
    API integration :api, after gate1, 5d, dependsOn gate1
    QA cycle :qa, after api, 4d, dependsOn api
    Launch prep :prep, after qa, 2d, dependsOn qa
    Go live :milestone, golive, after prep, 0d, dependsOn prep"#,
    },
    // ── Git Graph ───────────────────────────────────────────
    Example {
        name: "Commit flow diagram",
        category: "Git Graph",
        source: r#"gitGraph:
    commit "Ashish"
    branch newbranch
    checkout newbranch
    commit id:"1111"
    commit tag:"test"
    checkout main
    commit type: HIGHLIGHT
    commit
    merge newbranch
    commit
    branch b2
    commit"#,
    },
    // ── Mindmap ─────────────────────────────────────────────
    Example {
        name: "Mindmap",
        category: "Mindmap",
        source: r#"mindmap
  root((mindmap))
    Origins
      Long history
      ::icon(fa fa-book)
      Popularisation
        British popular psychology author Tony Buzan
    Research
      On effectiveness<br/>and features
      On Automatic creation
        Uses
            Creative techniques
            Strategic planning
            Argument mapping
    Tools
      Pen and paper
      Mermaid"#,
    },
    // ── Architecture ──────────────────────────────────────────
    Example {
        name: "Architecture diagram",
        category: "Architecture",
        source: r#"architecture-beta
  group internet(internet)[Internet]
  group app(server)[Application]
  group data(database)[Data Layer]
  group api(server)[API Layer] in app
  group jobs(server)[Worker Layer] in app
  service user(internet)[User] in internet
  service lb(server)[Load Balancer] in api
  service apiSvc(server)[API Service] in api
  service worker(server)[Background Worker] in jobs
  service db(database)[Postgres] in data
  service cache(disk)[Cache] in data
  user:R --> L:lb
  lb:R --> L:apiSvc
  apiSvc:R --> L:db
  apiSvc:B --> T:cache
  apiSvc:B --> T:worker
  worker:R --> L:db
"#,
    },
    Example {
        name: "Architecture diagram - Microservices platform",
        category: "Architecture",
        source: r#"architecture-beta
  group clients(internet)[Client Tier]
  group aws(cloud)[AWS]
  group platform(server)[Platform] in aws
  group edge(shield)[Edge Layer] in platform
  group services(layers)[Services] in platform
  group persistence(database)[Persistence] in aws

  service enduser(user)[End User] in clients
  service mobile(mobile)[Mobile App] in clients

  service cdn(network)[CDN] in edge
  service gw(api)[API Gateway] in edge
  service auth(lock)[Auth Service] in edge

  service userSvc(user)[User Service] in services
  service orderSvc(cpu)[Order Service] in services
  service notifSvc(zap)[Notifier] in services

  service pg(database)[PostgreSQL] in persistence
  service kv(disk)[Redis] in persistence
  service mq(layers)[Message Bus] in persistence

  enduser:R --> L:cdn
  mobile:R --> L:cdn
  cdn:R --> L:gw
  gw:B --> T:auth
  auth:B --> T:userSvc
  gw:R --> L:orderSvc
  userSvc:R --> L:pg
  orderSvc:R --> L:pg
  orderSvc:B --> T:kv
  orderSvc:R --> L:mq
  mq:B --> T:notifSvc
"#,
    },
    // ── ER Diagrams ──────────────────────────────────────────
    Example {
        name: "Basic ER diagram",
        category: "ER Diagram",
        source: r#"erDiagram
    CUSTOMER ||--o{ ORDER : places
    ORDER ||--|{ LINE-ITEM : contains
    CUSTOMER {
        string name PK
        string email UK
    }
    ORDER {
        int id PK
        date created
        int customerId FK
    }
    LINE-ITEM {
        int quantity
        float price
    }"#,
    },
    Example {
        name: "Non-identifying relationships",
        category: "ER Diagram",
        source: r#"erDiagram
    PERSON ||--o{ CAR : owns
    PERSON ||..o{ HOBBY : "interested in"
    CAR }o..o{ MECHANIC : "serviced by""#,
    },
    Example {
        name: "All cardinality types",
        category: "ER Diagram",
        source: r#"erDiagram
    A ||--|| B : "one to one"
    C ||--o| D : "one to zero-or-one"
    E ||--|{ F : "one to one-or-more"
    G ||--o{ H : "one to zero-or-more""#,
    },
    // ── State Diagrams ────────────────────────────────────────
    Example {
        name: "Simple state diagram",
        category: "State Diagram",
        source: r#"stateDiagram-v2
    [*] --> Still
    Still --> [*]
    Still --> Moving
    Moving --> Still
    Moving --> Crash
    Crash --> [*]"#,
    },
    Example {
        name: "Composite states with transitions",
        category: "State Diagram",
        source: r#"stateDiagram-v2
    [*] --> Active
    Active --> [*]

    state Active {
        [*] --> Idle
        Idle --> Processing : start
        Processing --> Idle : done
        Processing --> Error : fail
        Error --> Idle : retry
    }"#,
    },
    Example {
        name: "Fork, join, choice, and notes",
        category: "State Diagram",
        source: r#"stateDiagram-v2
    state fork_state <<fork>>
    state join_state <<join>>
    state if_state <<choice>>

    [*] --> fork_state
    fork_state --> TaskA
    fork_state --> TaskB

    TaskA --> join_state
    TaskB --> join_state

    join_state --> if_state

    if_state --> Success : passed
    if_state --> Failure : failed

    Success --> [*]
    Failure --> Retry
    Retry --> fork_state

    note right of if_state
        Evaluate results
        from both tasks
    end note"#,
    },
];

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn build_html(results: &[(&Example, Result<String, String>)]) -> String {
    let mut html = String::from(
        r#"<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="UTF-8">
<title>mermaid-rs Examples Comparison</title>
<style>
  * { box-sizing: border-box; margin: 0; padding: 0; }
  body { font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, sans-serif; background: #f5f5f5; padding: 20px; }
  h1 { text-align: center; margin-bottom: 24px; color: #333; }
  .example { background: #fff; border: 1px solid #ddd; border-radius: 8px; margin-bottom: 24px; overflow: hidden; }
  .category-header { background: #1a202c; color: #fff; padding: 14px 20px; font-size: 20px; font-weight: 700; margin-top: 32px; margin-bottom: 16px; border-radius: 8px; }
  .category-header:first-of-type { margin-top: 0; }
  .example-header { background: #2d3748; color: #fff; padding: 10px 16px; font-size: 16px; font-weight: 600; }
  .columns { display: grid; grid-template-columns: 1fr 1fr 1fr; min-height: 200px; }
  .col { padding: 12px; border-right: 1px solid #e2e8f0; overflow: auto; }
  .col:last-child { border-right: none; }
  .col-label { font-size: 11px; text-transform: uppercase; letter-spacing: 0.05em; color: #718096; margin-bottom: 8px; font-weight: 600; }
  textarea.source-input { background: #f7fafc; border: 1px solid #e2e8f0; border-radius: 4px; padding: 10px; font-size: 12px; font-family: "SF Mono", "Fira Code", monospace; white-space: pre; word-break: break-word; overflow: auto; max-height: 500px; width: 100%; min-height: 150px; resize: vertical; line-height: 1.4; tab-size: 4; }
  .error { background: #fff5f5; border: 1px solid #fc8181; border-radius: 4px; padding: 10px; color: #c53030; font-size: 13px; font-family: monospace; white-space: pre-wrap; }
  .svg-container svg { max-width: 100%; height: auto; }
  .wasm-status { text-align: center; padding: 8px; margin-bottom: 16px; border-radius: 4px; font-size: 13px; }
  .wasm-status.loading { background: #fffbeb; color: #92400e; }
  .wasm-status.ready { background: #ecfdf5; color: #065f46; }
  .wasm-status.error { background: #fff5f5; color: #c53030; }
</style>
</head>
<body>
<h1>mermaid-rs Examples Comparison</h1>
<div id="wasm-status" class="wasm-status loading">Loading WASM module...</div>
"#,
    );

    let mut current_category = "";
    for (i, (example, result)) in results.iter().enumerate() {
        if example.category != current_category {
            current_category = example.category;
            html.push_str(&format!(
                "<div class=\"category-header\">{}</div>\n",
                html_escape(current_category),
            ));
        }
        html.push_str(&format!(
            r#"<div class="example">
<div class="example-header">#{} &mdash; {}</div>
<div class="columns">
<div class="col">
  <div class="col-label">Input</div>
  <textarea class="source-input" data-example-id="ex-{}">{}</textarea>
</div>
<div class="col">
  <div class="col-label">mermaid-rs</div>
  <div class="error" id="mermaid-rs-error-{}" style="display:none;"></div>
  <div class="svg-container" id="mermaid-rs-{}">
"#,
            i + 1,
            html_escape(example.name),
            i,
            html_escape(example.source),
            i,
            i,
        ));

        match result {
            Ok(svg) => {
                html.push_str(svg);
            }
            Err(err) => {
                html.push_str(&format!(
                    "<div class=\"error\">{}</div>",
                    html_escape(err)
                ));
            }
        }

        html.push_str(&format!(
            r#"</div>
</div>
<div class="col">
  <div class="col-label">mermaid.js</div>
  <div class="error" id="mermaid-js-error-{}" style="display:none;"></div>
  <div id="mermaid-js-{}" class="svg-container"></div>
</div>
</div>
</div>
"#,
            i, i,
        ));
    }

    let example_count = results.len();

    html.push_str(&format!(
        r#"<script src="https://cdn.jsdelivr.net/npm/mermaid/dist/mermaid.min.js"></script>
<script>mermaid.initialize({{ startOnLoad: false }});</script>
<script type="module">
const EXAMPLE_COUNT = {};

// Render initial mermaid.js diagrams
async function renderInitialMermaidJs() {{
  for (let i = 0; i < EXAMPLE_COUNT; i++) {{
    const textarea = document.querySelector(`[data-example-id="ex-${{i}}"]`);
    const container = document.getElementById(`mermaid-js-${{i}}`);
    const errorDiv = document.getElementById(`mermaid-js-error-${{i}}`);
    if (!textarea || !container) continue;
    try {{
      const {{ svg }} = await mermaid.render(`mermaid-js-graph-${{i}}`, textarea.value);
      container.innerHTML = svg;
      errorDiv.style.display = 'none';
    }} catch (e) {{
      container.innerHTML = '';
      errorDiv.textContent = e.message || String(e);
      errorDiv.style.display = 'block';
    }}
  }}
}}

renderInitialMermaidJs();

// Load WASM module
let wasmModule = null;
let wasmError = null;
const statusEl = document.getElementById('wasm-status');

async function loadWasm() {{
  // Try normal ES module import (works when served over HTTP)
  try {{
    const wasm = await import('./wasm-pkg/mermaid_wasm.js');
    await wasm.default();
    return wasm;
  }} catch (e) {{
    console.warn('WASM import() failed, trying XHR fallback:', e);
  }}

  // Fallback: load .wasm bytes via XHR (works on file:// protocol)
  try {{
    const jsUrl = new URL('./wasm-pkg/mermaid_wasm.js', location.href).href;
    const wasm = await import(jsUrl);
    const wasmUrl = new URL('./wasm-pkg/mermaid_wasm_bg.wasm', location.href).href;
    const response = await new Promise((resolve, reject) => {{
      const xhr = new XMLHttpRequest();
      xhr.open('GET', wasmUrl);
      xhr.responseType = 'arraybuffer';
      xhr.onload = () => xhr.status === 200 || xhr.status === 0
        ? resolve(xhr.response)
        : reject(new Error(`HTTP ${{xhr.status}}`));
      xhr.onerror = () => reject(new Error('XHR failed'));
      xhr.send();
    }});
    wasm.initSync({{ module: new WebAssembly.Module(new Uint8Array(response)) }});
    return wasm;
  }} catch (e) {{
    console.warn('WASM XHR fallback also failed:', e);
    throw e;
  }}
}}

try {{
  wasmModule = await loadWasm();
  statusEl.textContent = 'WASM loaded — edits will live-update both renderers';
  statusEl.className = 'wasm-status ready';
}} catch (e) {{
  wasmError = 'WASM not available. Run: make serve-examples (file:// does not support WASM fetch)';
  statusEl.textContent = wasmError;
  statusEl.className = 'wasm-status error';
  console.warn('WASM load failed:', e);
}}

// Live re-render on textarea edit
let mermaidJsCounter = EXAMPLE_COUNT;

document.querySelectorAll('textarea.source-input').forEach(textarea => {{
  const id = textarea.dataset.exampleId;
  const idx = id.replace('ex-', '');

  textarea.addEventListener('input', async () => {{
    const source = textarea.value;

    // Re-render mermaid-rs via WASM
    const rsContainer = document.getElementById(`mermaid-rs-${{idx}}`);
    const rsError = document.getElementById(`mermaid-rs-error-${{idx}}`);
    if (rsContainer) {{
      if (wasmModule) {{
        try {{
          const svg = wasmModule.render_svg(source);
          rsContainer.innerHTML = svg;
          rsError.style.display = 'none';
        }} catch (e) {{
          rsContainer.innerHTML = '';
          rsError.textContent = e.message || String(e);
          rsError.style.display = 'block';
        }}
      }} else {{
        rsError.textContent = wasmError || 'WASM not loaded';
        rsError.style.display = 'block';
      }}
    }}

    // Re-render mermaid.js
    const jsContainer = document.getElementById(`mermaid-js-${{idx}}`);
    const jsError = document.getElementById(`mermaid-js-error-${{idx}}`);
    if (jsContainer) {{
      try {{
        const graphId = `mermaid-js-live-${{mermaidJsCounter++}}`;
        const {{ svg }} = await mermaid.render(graphId, source);
        jsContainer.innerHTML = svg;
        jsError.style.display = 'none';
      }} catch (e) {{
        jsContainer.innerHTML = '';
        jsError.textContent = e.message || String(e);
        jsError.style.display = 'block';
      }}
    }}
  }});
}});
</script>
</body>
</html>
"#,
        example_count,
    ));

    html
}

#[test]
fn generate_examples_comparison() {
    let config = RenderConfig::default();

    let results: Vec<(&Example, Result<String, String>)> = EXAMPLES
        .iter()
        .map(|ex| {
            let result: Result<String, String> = match render(ex.source, &config) {
                Ok(output) => output.into_svg().map_err(|e| format!("{}", e)),
                Err(e) => Err(format!("{}", e)),
            };
            (ex, result)
        })
        .collect();

    // Print summary
    let passed = results.iter().filter(|(_, r)| r.is_ok()).count();
    let failed = results.iter().filter(|(_, r)| r.is_err()).count();
    println!("\n=== Examples Comparison ===");
    println!("{} rendered, {} errored\n", passed, failed);

    for (ex, result) in &results {
        let status = if result.is_ok() { "OK" } else { "ERR" };
        println!("  [{}] {}", status, ex.name);
    }

    let html = build_html(&results);
    let out_path = std::path::Path::new("target/examples-comparison.html");
    fs::create_dir_all(out_path.parent().unwrap()).unwrap();
    fs::write(out_path, &html).unwrap();

    println!(
        "\nHTML comparison written to: {}\n",
        fs::canonicalize(out_path).unwrap().display()
    );
}
