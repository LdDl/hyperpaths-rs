/// Link is an edge in the transit network graph.
#[derive(Debug, Clone, PartialEq)]
pub struct Link {
    /// Source node of the link
    pub from_node: String,
    /// Target node of the link
    pub to_node: String,
    /// Corresponding route
    pub route_id: String,
    /// Travel time along the link (in minutes or any consistent unit)
    pub travel_cost: f64,
    /// Service headway. Boarding links have headway > 0 (frequency = 1/headway).
    /// On-board (riding) links have headway = 0 (no waiting).
    pub headway: f64,
}

impl Link {
    /// Convenience constructor
    pub fn new(
        from_node: &str,
        to_node: &str,
        route_id: &str,
        travel_cost: f64,
        headway: f64,
    ) -> Self {
        Link {
            from_node: from_node.to_string(),
            to_node: to_node.to_string(),
            route_id: route_id.to_string(),
            travel_cost,
            headway,
        }
    }
}
