export interface Example {
  name: string;
  category: string;
  code: string;
}

export const examples: Example[] = [
  // ── Flowcharts ──────────────────────────────────────────
  {
    name: "Basic flowchart",
    category: "Flowchart",
    code: `graph LR
    A[Square Rect] -- Link text --> B((Circle))
    A --> C(Round Rect)
    B --> D{Rhombus}
    C --> D`,
  },
  {
    name: "Larger flowchart with styling",
    category: "Flowchart",
    code: `graph TB
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
     class di orange`,
  },
  {
    name: "LR Dependency diagram",
    category: "Flowchart",
    code: `flowchart LR
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
    RTQ --> DL[Data Localization]`,
  },
  {
    name: "TD Dependency diagram",
    category: "Flowchart",
    code: `flowchart TD
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
    RTQ --> DL[Data Localization]`,
  },
  {
    name: "Complex Organization Flowchart",
    category: "Flowchart",
    code: `graph TD
    subgraph Platform
        subgraph OrgPkg["Organization: Lorem Corp<br/>(id=org-001, plan=enterprise)"]
            RootOU["<b>Root OU: Lorem US</b><br/>id = ou-root-001<br/>geo = US<br/>org_id = org-aaa"]
            EUOU["<b>Child OU: Lorem EU</b><br/>id = ou-child-001<br/>geo = EU<br/>org_id = org-bbb"]
            APACOU["<b>Child OU: Lorem APAC</b><br/>id = ou-child-002<br/>geo = US<br/>org_id = null"]
            RootOU -->|HAS_CHILD_OU| EUOU
            RootOU -->|HAS_CHILD_OU| APACOU
        end

        subgraph OrgPkg2["Organization: Ipsum Inc<br/>(id=org-002, plan=free)"]
            SmallOU["<b>Root OU: Ipsum Inc</b><br/>id = ou-root-002<br/>geo = US<br/>org_id = org-ccc"]
        end

        subgraph LEPkg1["Legal Entity: Lorem Corp Holdings"]
            LE1["<b>Lorem Corp Holdings</b><br/>id = le-001<br/>entityType = CORPORATION"]
            D1["lorem.com"]
            LE1 -->|OWNS_DOMAIN| D1
        end

        subgraph LEPkg2["Legal Entity: Lorem EU GmbH"]
            LE2["<b>Lorem EU GmbH</b><br/>id = le-002<br/>entityType = LLC"]
            D2["lorem.eu"]
            LE2 -->|OWNS_DOMAIN| D2
        end

        subgraph LEPkg3["Legal Entity: Ipsum LLC"]
            LE3["<b>Ipsum LLC</b><br/>id = le-003<br/>entityType = LLC"]
            D3["ipsum.io"]
            LE3 -->|OWNS_DOMAIN| D3
        end

        RootOU -.->|REPRESENTS| LE1
        EUOU -.->|REPRESENTS| LE2
        SmallOU -.->|REPRESENTS| LE3
    end

    subgraph OryNetwork["Identity Platform"]
        subgraph OryUS["Org: Lorem US (org-aaa)"]
            OO1["<b>Organization</b><br/>id = org-aaa<br/>label = Lorem US<br/>domains = lorem.com"]
            SSO1["<b>SAML Connection</b><br/>provider = Provider A"]
            ID1["Identity: user1@lorem.com"]
            ID2["Identity: user2@lorem.com"]
            OO1 -->|SSO provider| SSO1
            OO1 -->|member| ID1
            OO1 -->|member| ID2
        end

        subgraph OryEU["Org: Lorem EU (org-bbb)"]
            OO2["<b>Organization</b><br/>id = org-bbb<br/>label = Lorem EU<br/>domains = lorem.eu"]
            SSO2["<b>OIDC Connection</b><br/>provider = Provider B"]
            ID3["Identity: user3@lorem.eu"]
            OO2 -->|SSO provider| SSO2
            OO2 -->|member| ID3
        end

        subgraph OrySmall["Org: Ipsum Inc (org-ccc)"]
            OO3["<b>Organization</b><br/>id = org-ccc<br/>label = Ipsum Inc<br/>domains = ipsum.io"]
            ID4["Identity: user4@ipsum.io"]
            ID5["Identity: user5@ipsum.io"]
            OO3 -->|member| ID4
            OO3 -->|member| ID5
        end
    end

    subgraph ExtIdPs["External Identity Providers"]
        OktaIdP["Provider A IdP"]
        AzureIdP["Provider B IdP"]
    end

    %% Cross-references
    RootOU ==>|org_id| OO1
    EUOU ==>|org_id| OO2
    APACOU -.->|inherits from parent| OO1
    SmallOU ==>|org_id| OO3

    D1 -->|configures domain| OO1
    D2 -->|configures domain| OO2
    D3 -->|configures domain| OO3

    SSO1 -->|SAML AuthnRequest| OktaIdP
    SSO2 -->|OIDC /authorize| AzureIdP

    %% Styles
    style OryUS fill:#d4eaff,stroke:#336
    style OryEU fill:#d4f5d4,stroke:#363
    style OrySmall fill:#f5e6ff,stroke:#636
    style APACOU fill:#fff3cd,stroke:#996
    style OktaIdP fill:#f5f5f5,stroke:#999
    style AzureIdP fill:#f5f5f5,stroke:#999`,
  },
  {
    name: "Multi Subgraph Flowchart",
    category: "Flowchart",
    code: `graph TD
    subgraph RBAC["RBAC Layer"]
        Role_analyst["Role: analyst"]
        Role_editor["Role: editor"]
        Bob["User: bob"] -->|member of| Role_analyst
        Carol["User: carol"] -->|member of| Role_editor
    end

    subgraph Folders["Folder Hierarchy"]
        Root["Folder: root"]
        Eng["Folder: engineering"]
        Backend["Folder: backend"]

        Backend -->|parents| Eng
        Eng -->|parents| Root
    end

    subgraph Files["Files"]
        F1["design-doc.pdf"]
        F2["api-spec.yaml"]
        F3["secret-report.pdf"]

        F1 -->|parents| Backend
        F2 -->|parents| Backend
        F3 -->|parents| Eng
    end

    subgraph DirectGrants["Direct Entity Grants"]
        Alice["User: alice"]
        Alice -->|"viewers (direct)"| F3
    end

    Role_analyst -->|"viewers (RBAC)"| Root
    Role_editor -->|"editors (RBAC)"| Eng`,
  },
  {
    name: "Complex Nested Subgraphs (CI/CD)",
    category: "Flowchart",
    code: `graph TD
    subgraph CI["CI/CD Pipeline"]
        Commit["git push"] --> Build["Build"]
        Build --> Lint["Lint"]
        Build --> Unit["Unit Tests"]
        Build --> Integration["Integration Tests"]
        Lint --> Gate["Quality Gate"]
        Unit --> Gate
        Integration --> Gate
    end

    subgraph Staging["Staging Environment"]
        subgraph StageFE["Frontend"]
            SCDN["CDN"]
            SWeb["Web App"]
            SWeb --> SCDN
        end
        subgraph StageBE["Backend"]
            SAPI["API Gateway"]
            SAuth["Auth Service"]
            SQueue["Job Queue"]
            SAPI --> SAuth
            SAPI --> SQueue
        end
        subgraph StageData["Data Layer"]
            SPG["PostgreSQL"]
            SRedis["Redis"]
        end
        SWeb -->|API calls| SAPI
        SAuth --> SPG
        SQueue --> SRedis
    end

    subgraph Prod["Production Environment"]
        subgraph ProdFE["Frontend"]
            PCDN["CDN"]
            PWA["Web App"]
            PPWA["PWA Shell"]
            PWA --> PCDN
            PPWA --> PCDN
        end
        subgraph ProdBE["Backend"]
            PAPI["API Gateway"]
            PAuth["Auth Service"]
            PQueue["Job Queue"]
            PNotify["Notifications"]
            PAPI --> PAuth
            PAPI --> PQueue
            PQueue --> PNotify
        end
        subgraph ProdData["Data Layer"]
            PPrimary["PG Primary"]
            PReplica["PG Replica"]
            PRedis["Redis Cluster"]
            PS3["Object Store"]
            PPrimary --> PReplica
        end
        PWA -->|API calls| PAPI
        PAuth --> PPrimary
        PQueue --> PRedis
        PQueue --> PS3
        PAPI -.->|read replica| PReplica
    end

    subgraph External["External Services"]
        Stripe["Stripe"]
        SendGrid["SendGrid"]
        Datadog["Datadog"]
    end

    subgraph Monitoring["Observability"]
        Grafana["Grafana"]
        Alerts["PagerDuty"]
        Grafana --> Alerts
    end

    Gate -->|deploy staging| SAPI
    Gate -->|deploy prod| PAPI
    PNotify --> SendGrid
    PAPI -.->|payments| Stripe
    SAuth -.->|metrics| Datadog
    PAuth -.->|metrics| Datadog
    Datadog --> Grafana`,
  },
  // ── Sequence Diagrams ───────────────────────────────────
  {
    name: "Basic sequence diagram",
    category: "Sequence Diagram",
    code: `sequenceDiagram
    Alice ->> Bob: Hello Bob, how are you?
    Bob-->>John: How about you John?
    Bob--x Alice: I am good thanks!
    Bob-x John: I am good thanks!
    Note right of John: Bob thinks a long<br/>long time, so long<br/>that the text does<br/>not fit on a row.

    Bob-->Alice: Checking with John...
    Alice->John: Yes... John, how are you?`,
  },
  {
    name: "Loops, alt and opt",
    category: "Sequence Diagram",
    code: `sequenceDiagram
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
    end`,
  },
  {
    name: "Message to self in loop",
    category: "Sequence Diagram",
    code: `sequenceDiagram
    participant Alice
    participant Bob
    Alice->>John: Hello John, how are you?
    loop HealthCheck
        John->>John: Fight against hypochondria
    end
    Note right of John: Rational thoughts<br/>prevail...
    John-->>Alice: Great!
    John->>Bob: How about you?
    Bob-->>John: Jolly good!`,
  },
  {
    name: "Blogging app service communication",
    category: "Sequence Diagram",
    code: `sequenceDiagram
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
    end`,
  },
  // ── Pie Charts ──────────────────────────────────────────
  {
    name: "Basic Pie Chart (Netflix)",
    category: "Pie Chart",
    code: `pie title NETFLIX
         "Time spent looking for movie" : 90
         "Time spent watching it" : 10`,
  },
  {
    name: "Basic Pie Chart (Voldemort)",
    category: "Pie Chart",
    code: `pie title What Voldemort doesn't have?
         "FRIENDS" : 2
         "FAMILY" : 3
         "NOSE" : 45`,
  },
  // ── Gantt Charts ────────────────────────────────────────
  {
    name: "Gantt chart",
    category: "Gantt Chart",
    code: `gantt
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
    Important milestone :crit, milestone, 2014-01-12, 0d`,
  },
  {
    name: "Gantt chart - complex dependencies",
    category: "Gantt Chart",
    code: `gantt
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
    Go live :milestone, golive, after prep, 0d, dependsOn prep`,
  },
  // ── Git Graph ───────────────────────────────────────────
  {
    name: "Commit flow diagram",
    category: "Git Graph",
    code: `gitGraph:
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
    commit`,
  },
  // ── Mindmap ─────────────────────────────────────────────
  {
    name: "Mindmap",
    category: "Mindmap",
    code: `mindmap
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
      Mermaid`,
  },
  // ── Architecture ──────────────────────────────────────────
  {
    name: "Architecture diagram",
    category: "Architecture",
    code: `architecture-beta
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
  worker:R --> L:db`,
  },
  {
    name: "Microservices platform",
    category: "Architecture",
    code: `architecture-beta
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
  mq:B --> T:notifSvc`,
  },
  // ── ER Diagrams ──────────────────────────────────────────
  {
    name: "Basic ER diagram",
    category: "ER Diagram",
    code: `erDiagram
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
    }`,
  },
  {
    name: "Non-identifying relationships",
    category: "ER Diagram",
    code: `erDiagram
    PERSON ||--o{ CAR : owns
    PERSON ||..o{ HOBBY : "interested in"
    CAR }o..o{ MECHANIC : "serviced by"`,
  },
  {
    name: "All cardinality types",
    category: "ER Diagram",
    code: `erDiagram
    A ||--|| B : "one to one"
    C ||--o| D : "one to zero-or-one"
    E ||--|{ F : "one to one-or-more"
    G ||--o{ H : "one to zero-or-more"`,
  },
  // ── State Diagrams ────────────────────────────────────────
  {
    name: "Simple state diagram",
    category: "State Diagram",
    code: `stateDiagram-v2
    [*] --> Still
    Still --> [*]
    Still --> Moving
    Moving --> Still
    Moving --> Crash
    Crash --> [*]`,
  },
  {
    name: "Composite states with transitions",
    category: "State Diagram",
    code: `stateDiagram-v2
    [*] --> Active
    Active --> [*]

    state Active {
        [*] --> Idle
        Idle --> Processing : start
        Processing --> Idle : done
        Processing --> Error : fail
        Error --> Idle : retry
    }`,
  },
  {
    name: "Fork, join, choice, and notes",
    category: "State Diagram",
    code: `stateDiagram-v2
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
    end note`,
  },
];

export function getExampleByName(name: string): Example | undefined {
  return examples.find(e => e.name === name);
}
