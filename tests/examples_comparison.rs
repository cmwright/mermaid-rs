//! Integration test that renders all examples from the Mermaid examples page
//! and generates an HTML comparison page showing input, mermaid-rs output, and mermaid.js output.

use mermaid_core::{render, RenderConfig};
use std::fs;

struct Example {
    name: &'static str,
    source: &'static str,
}

const EXAMPLES: &[Example] = &[
    Example {
        name: "Test Loop - Complex Organization Flowchart",
        source: include_str!("../tests/test_loop/input_mermaid.mmd"),
    },
    Example {
        name: "Basic Pie Chart (Netflix)",
        source: r#"pie title NETFLIX
         "Time spent looking for movie" : 90
         "Time spent watching it" : 10"#,
    },
    Example {
        name: "Basic Pie Chart (Voldemort)",
        source: r#"pie title What Voldemort doesn't have?
         "FRIENDS" : 2
         "FAMILY" : 3
         "NOSE" : 45"#,
    },
    Example {
        name: "Basic sequence diagram",
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
        name: "Basic flowchart",
        source: r#"graph LR
    A[Square Rect] -- Link text --> B((Circle))
    A --> C(Round Rect)
    B --> D{Rhombus}
    C --> D"#,
    },
    Example {
        name: "Larger flowchart with styling",
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
        name: "Loops, alt and opt",
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
    Example {
        name: "Mindmap",
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
    Example {
        name: "Commit flow diagram",
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
    Example {
        name: "Gantt chart",
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
        name: "LR Flowchart - Dependency diagram with dotted edges",
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
        name: "Gantt chart - complex dependency handling",
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
  .example-header { background: #2d3748; color: #fff; padding: 10px 16px; font-size: 16px; font-weight: 600; }
  .columns { display: grid; grid-template-columns: 1fr 1fr 1fr; min-height: 200px; }
  .col { padding: 12px; border-right: 1px solid #e2e8f0; overflow: auto; }
  .col:last-child { border-right: none; }
  .col-label { font-size: 11px; text-transform: uppercase; letter-spacing: 0.05em; color: #718096; margin-bottom: 8px; font-weight: 600; }
  pre.source { background: #f7fafc; border: 1px solid #e2e8f0; border-radius: 4px; padding: 10px; font-size: 12px; font-family: "SF Mono", "Fira Code", monospace; white-space: pre-wrap; word-break: break-word; overflow: auto; max-height: 500px; }
  .error { background: #fff5f5; border: 1px solid #fc8181; border-radius: 4px; padding: 10px; color: #c53030; font-size: 13px; font-family: monospace; white-space: pre-wrap; }
  .svg-container svg { max-width: 100%; height: auto; }
</style>
</head>
<body>
<h1>mermaid-rs Examples Comparison</h1>
"#,
    );

    for (i, (example, result)) in results.iter().enumerate() {
        html.push_str(&format!(
            r#"<div class="example">
<div class="example-header">#{} &mdash; {}</div>
<div class="columns">
<div class="col">
  <div class="col-label">Input</div>
  <pre class="source">{}</pre>
</div>
<div class="col">
  <div class="col-label">mermaid-rs</div>
"#,
            i + 1,
            html_escape(example.name),
            html_escape(example.source),
        ));

        match result {
            Ok(svg) => {
                html.push_str(&format!("  <div class=\"svg-container\">{}</div>\n", svg));
            }
            Err(err) => {
                html.push_str(&format!(
                    "  <div class=\"error\">{}</div>\n",
                    html_escape(err)
                ));
            }
        }

        html.push_str(&format!(
            r#"</div>
<div class="col">
  <div class="col-label">mermaid.js</div>
  <pre class="mermaid">{}</pre>
</div>
</div>
</div>
"#,
            html_escape(example.source),
        ));
    }

    html.push_str(
        r#"<script src="https://cdn.jsdelivr.net/npm/mermaid/dist/mermaid.min.js"></script>
<script>mermaid.initialize({ startOnLoad: true });</script>
</body>
</html>
"#,
    );

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
