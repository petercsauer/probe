use prb_tui::filter_persistence::FilterPersistence;
use prb_tui::filter_state::FilterState;

#[test]
fn test_default_templates_count() {
    let templates = FilterPersistence::default_templates();
    assert!(
        templates.len() >= 10,
        "Should have at least 10 templates, found {}",
        templates.len()
    );
}

#[test]
fn test_template_categories() {
    let templates = FilterPersistence::default_templates();
    let valid_categories = ["Protocol", "Performance", "Security", "Network"];

    for template in &templates {
        assert!(
            valid_categories.contains(&template.category.as_str()),
            "Template '{}' has invalid category: {}",
            template.name,
            template.category
        );
    }
}

#[test]
fn test_all_templates_have_required_fields() {
    let templates = FilterPersistence::default_templates();

    for template in &templates {
        assert!(
            !template.name.is_empty(),
            "Template name should not be empty"
        );
        assert!(
            !template.category.is_empty(),
            "Template '{}' category should not be empty",
            template.name
        );
        assert!(
            !template.filter.is_empty(),
            "Template '{}' filter should not be empty",
            template.name
        );
        assert!(
            !template.description.is_empty(),
            "Template '{}' description should not be empty",
            template.name
        );
        assert!(
            !template.tags.is_empty(),
            "Template '{}' should have at least one tag",
            template.name
        );
    }
}

#[test]
fn test_search_by_name() {
    let persistence = FilterPersistence::default();
    let results = persistence.search_templates("dns");

    assert!(!results.is_empty(), "Should find DNS template by name");
    assert!(
        results.iter().any(|t| t.name.contains("DNS")),
        "Should find template with 'DNS' in name"
    );
}

#[test]
fn test_search_by_tag() {
    let persistence = FilterPersistence::default();
    let results = persistence.search_templates("protocol");

    assert!(
        !results.is_empty(),
        "Should find templates with 'protocol' tag"
    );
    assert!(
        results
            .iter()
            .any(|t| t.tags.iter().any(|tag| tag.contains("protocol"))),
        "Should find templates tagged with 'protocol'"
    );
}

#[test]
fn test_search_by_description() {
    let persistence = FilterPersistence::default();
    let results = persistence.search_templates("fragmentation");

    assert!(!results.is_empty(), "Should find template by description");
    assert!(
        results
            .iter()
            .any(|t| t.description.to_lowercase().contains("fragmentation")),
        "Should find template with 'fragmentation' in description"
    );
}

#[test]
fn test_search_case_insensitive() {
    let persistence = FilterPersistence::default();
    let lower_results = persistence.search_templates("dns");
    let upper_results = persistence.search_templates("DNS");
    let mixed_results = persistence.search_templates("DnS");

    assert_eq!(
        lower_results.len(),
        upper_results.len(),
        "Search should be case-insensitive"
    );
    assert_eq!(
        lower_results.len(),
        mixed_results.len(),
        "Search should be case-insensitive"
    );
}

#[test]
fn test_search_no_match() {
    let persistence = FilterPersistence::default();
    let results = persistence.search_templates("nonexistent_filter_xyz");

    assert!(results.is_empty(), "Should return empty for no matches");
}

#[test]
fn test_search_empty_query_returns_all() {
    let persistence = FilterPersistence::default();
    let all_templates = persistence.get_templates();
    let empty_search = persistence.search_templates("");

    assert_eq!(
        all_templates.len(),
        empty_search.len(),
        "Empty search should return all templates"
    );
}

#[test]
fn test_apply_template() {
    let mut state = FilterState::new_with_persistence(false);
    let templates = state.get_templates();

    // Find the DNS template
    let dns_template = templates
        .iter()
        .find(|t| t.name.contains("DNS"))
        .expect("Should have DNS template");

    // Apply the template
    state.apply_template(dns_template);

    // Verify the filter text was set
    assert_eq!(
        state.text, dns_template.filter,
        "apply_template should set filter text"
    );
}

#[test]
fn test_specific_templates_exist() {
    let templates = FilterPersistence::default_templates();

    // Check for specific templates mentioned in the spec
    let template_names: Vec<&str> = templates.iter().map(|t| t.name.as_str()).collect();

    assert!(
        template_names.iter().any(|&name| name.contains("DNS")),
        "Should have DNS template"
    );
    assert!(
        template_names.iter().any(|&name| name.contains("TLS")),
        "Should have TLS template"
    );
    assert!(
        template_names.iter().any(|&name| name.contains("gRPC")),
        "Should have gRPC template"
    );
    assert!(
        template_names.iter().any(|&name| name.contains("HTTP")),
        "Should have HTTP template"
    );
    assert!(
        template_names
            .iter()
            .any(|&name| name.contains("Large Frames")),
        "Should have Large Frames template"
    );
}

#[test]
fn test_grpc_template() {
    let persistence = FilterPersistence::default();
    let results = persistence.search_templates("grpc");

    assert!(!results.is_empty(), "Should find gRPC template");

    let grpc_template = results
        .iter()
        .find(|t| t.name.contains("gRPC"))
        .expect("Should have gRPC template");

    assert_eq!(
        grpc_template.filter, r#"transport == "grpc""#,
        "gRPC template should have correct filter"
    );
}

#[test]
fn test_template_filter_syntax_validity() {
    let templates = FilterPersistence::default_templates();

    // Basic syntax validation - ensure filters are not empty and have reasonable structure
    for template in &templates {
        let filter = &template.filter;

        // Filter should not be empty
        assert!(!filter.is_empty(), "Filter should not be empty");

        // Filter should not have unbalanced quotes
        let quote_count = filter.chars().filter(|&c| c == '"').count();
        assert_eq!(
            quote_count % 2,
            0,
            "Template '{}' has unbalanced quotes in filter: {}",
            template.name,
            filter
        );

        // Filter should not have syntax errors like unmatched braces
        let open_braces = filter.chars().filter(|&c| c == '{').count();
        let close_braces = filter.chars().filter(|&c| c == '}').count();
        assert_eq!(
            open_braces, close_braces,
            "Template '{}' has unmatched braces in filter: {}",
            template.name, filter
        );
    }
}

#[test]
fn test_get_templates() {
    let persistence = FilterPersistence::default();
    let templates = persistence.get_templates();

    assert!(
        !templates.is_empty(),
        "get_templates should return templates"
    );
    assert_eq!(
        templates.len(),
        FilterPersistence::default_templates().len(),
        "get_templates should return all default templates"
    );
}

#[test]
fn test_filter_state_template_methods() {
    let state = FilterState::new_with_persistence(false);

    // Test get_templates
    let templates = state.get_templates();
    assert!(!templates.is_empty(), "Should return templates");

    // Test search_templates
    let dns_results = state.search_templates("dns");
    assert!(!dns_results.is_empty(), "Should find DNS templates");
}

#[test]
fn test_multiple_category_distribution() {
    let templates = FilterPersistence::default_templates();

    // Count templates in each category
    let mut category_counts = std::collections::HashMap::new();
    for template in &templates {
        *category_counts.entry(&template.category).or_insert(0) += 1;
    }

    // Should have templates in multiple categories
    assert!(
        category_counts.len() >= 3,
        "Should have templates in at least 3 different categories"
    );
}

#[test]
fn test_template_uniqueness() {
    let templates = FilterPersistence::default_templates();

    // Check that template names are unique
    let mut names = std::collections::HashSet::new();
    for template in &templates {
        assert!(
            names.insert(&template.name),
            "Template name '{}' is not unique",
            template.name
        );
    }

    // Check that filters are unique (different templates should have different filters)
    let mut filters = std::collections::HashSet::new();
    for template in &templates {
        assert!(
            filters.insert(&template.filter),
            "Filter '{}' is not unique",
            template.filter
        );
    }
}
