use mermaid_core::parser::flowchart::parse_flowchart;
use mermaid_core::layout::flowchart::graph_builder;
use mermaid_core::{render, RenderConfig};

fn main() {
    let source = r#"graph TD
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
    Role_editor -->|"editors (RBAC)"| Eng"#;

    let ast = parse_flowchart(source).unwrap();

    println!("=== PARSED AST ===\n");

    println!("Top-level nodes:");
    for node in &ast.nodes {
        println!("  {} (label: {:?})", node.id, node.label);
    }

    println!("\nTop-level edges:");
    for edge in &ast.edges {
        println!("  {} -> {} (label: {:?})", edge.from, edge.to, edge.label);
    }

    println!("\nSubgraphs:");
    for sg in &ast.subgraphs {
        println!("\n  Subgraph: {} (label: {:?})", sg.id, sg.label);
        println!("  Nodes:");
        for node in &sg.nodes {
            println!("    {} (label: {:?}, shape: {:?})", node.id, node.label, node.shape);
        }
        println!("  Edges:");
        for edge in &sg.edges {
            println!("    {} -> {} (label: {:?})", edge.from, edge.to, edge.label);
        }
    }

    println!("\n=== SUBGRAPH MEMBERSHIP ===\n");
    let membership = graph_builder::build_subgraph_membership(&ast);
    let mut entries: Vec<_> = membership.iter().collect();
    entries.sort_by_key(|(id, _)| id.as_str());
    for (node_id, path) in &entries {
        println!("  {} -> {:?}", node_id, path);
    }

    // Highlight the problems
    println!("\n=== POTENTIAL ISSUES ===\n");

    // Check for nodes that appear in multiple subgraphs' node lists
    for sg in &ast.subgraphs {
        for node in &sg.nodes {
            // Check if this node also appears in another subgraph
            for other_sg in &ast.subgraphs {
                if other_sg.id != sg.id {
                    if other_sg.nodes.iter().any(|n| n.id == node.id) {
                        println!(
                            "  WARNING: Node '{}' appears in BOTH subgraph '{}' and '{}'",
                            node.id, sg.id, other_sg.id
                        );
                    }
                }
            }
        }
    }

    // Check for membership mismatches
    for sg in &ast.subgraphs {
        for node in &sg.nodes {
            if let Some(path) = membership.get(&node.id) {
                if !path.is_empty() && path.last().unwrap() != &sg.id {
                    // Node is in this subgraph's node list but membership says different
                    let has_label_here = node.label.is_some();
                    println!(
                        "  MISMATCH: Node '{}' is in subgraph '{}' node list (has_label: {}) but membership says {:?}",
                        node.id, sg.id, has_label_here, path
                    );
                }
            }
        }
    }

    // Render the SVG
    println!("\n=== RENDERING SVG ===\n");
    let config = RenderConfig::default();
    match render(source, &config) {
        Ok(output) => {
            let svg = output.into_svg().unwrap();
            std::fs::write("/tmp/broken_flowchart_fixed.svg", &svg).unwrap();
            println!("SVG saved to /tmp/broken_flowchart_fixed.svg");
            println!("SVG length: {} bytes", svg.len());
        }
        Err(e) => {
            println!("ERROR: {}", e);
        }
    }
}
