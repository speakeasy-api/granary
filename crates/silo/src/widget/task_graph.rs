//! Task dependency graph visualization
//!
//! Renders task dependencies as a directed graph using petgraph
//! for layout calculation and iced Canvas for rendering.

use crate::appearance::{Palette, palette};
use granary_types::{Task as GranaryTask, TaskDependency, TaskStatus};
use iced::mouse;
use iced::widget::Canvas;
use iced::widget::canvas::{self, Cache, Frame, Geometry, Path, Stroke, Text};
use iced::{Color, Element, Length, Point, Rectangle, Renderer, Theme};
use petgraph::graph::{DiGraph, NodeIndex};
use petgraph::visit::Topo;
use std::collections::HashMap;

/// Node in the task graph
#[derive(Debug, Clone)]
pub struct TaskNode {
    pub task: GranaryTask,
    pub position: Point,
    pub level: usize,
}

/// Task dependency graph widget
pub struct TaskGraph {
    nodes: Vec<TaskNode>,
    edges: Vec<(usize, usize)>,
    cache: Cache,
    selected: Option<String>,
}

impl TaskGraph {
    /// Build graph from tasks and dependencies
    pub fn new(tasks: &[GranaryTask], dependencies: &[TaskDependency]) -> Self {
        let mut graph: DiGraph<String, ()> = DiGraph::new();
        let mut node_map: HashMap<String, NodeIndex> = HashMap::new();

        // Add nodes
        for task in tasks {
            let idx = graph.add_node(task.id.clone());
            node_map.insert(task.id.clone(), idx);
        }

        // Add edges (dependency -> task)
        for dep in dependencies {
            if let (Some(&from), Some(&to)) = (
                node_map.get(&dep.depends_on_task_id),
                node_map.get(&dep.task_id),
            ) {
                graph.add_edge(from, to, ());
            }
        }

        // Calculate levels using topological sort
        let mut levels: HashMap<String, usize> = HashMap::new();
        let mut topo = Topo::new(&graph);
        while let Some(idx) = topo.next(&graph) {
            let task_id = &graph[idx];
            let max_parent_level = graph
                .neighbors_directed(idx, petgraph::Direction::Incoming)
                .map(|p| levels.get(&graph[p]).copied().unwrap_or(0))
                .max()
                .unwrap_or(0);

            let level = if graph
                .neighbors_directed(idx, petgraph::Direction::Incoming)
                .count()
                > 0
            {
                max_parent_level + 1
            } else {
                0
            };
            levels.insert(task_id.clone(), level);
        }

        // Group tasks by level for horizontal positioning
        let mut level_counts: HashMap<usize, usize> = HashMap::new();
        let mut positions: HashMap<String, (usize, usize)> = HashMap::new();

        for (task_id, &level) in &levels {
            let count = level_counts.entry(level).or_insert(0);
            positions.insert(task_id.clone(), (level, *count));
            *count += 1;
        }

        // Create positioned nodes
        let h_spacing = 220.0;
        let v_spacing = 100.0;

        let nodes: Vec<TaskNode> = tasks
            .iter()
            .filter_map(|task| {
                let (level, index) = positions.get(&task.id)?;

                let x = *index as f32 * h_spacing + h_spacing / 2.0;
                let y = *level as f32 * v_spacing + v_spacing / 2.0;

                Some(TaskNode {
                    task: task.clone(),
                    position: Point::new(x, y),
                    level: *level,
                })
            })
            .collect();

        // Create edges
        let node_indices: HashMap<String, usize> = nodes
            .iter()
            .enumerate()
            .map(|(i, n)| (n.task.id.clone(), i))
            .collect();

        let edges: Vec<(usize, usize)> = dependencies
            .iter()
            .filter_map(|dep| {
                let from = node_indices.get(&dep.depends_on_task_id)?;
                let to = node_indices.get(&dep.task_id)?;
                Some((*from, *to))
            })
            .collect();

        Self {
            nodes,
            edges,
            cache: Cache::new(),
            selected: None,
        }
    }

    pub fn selected(mut self, task_id: Option<String>) -> Self {
        self.selected = task_id;
        self.cache.clear();
        self
    }

    fn draw_node(&self, frame: &mut Frame, node: &TaskNode, palette: &Palette) {
        let pos = node.position;
        let width = 160.0;
        let height = 50.0;
        let radius = 8.0;

        let status = node.task.status_enum();
        let (bg_color, border_color) = match status {
            TaskStatus::Done => (
                Color::from_rgba(
                    palette.status_done.r,
                    palette.status_done.g,
                    palette.status_done.b,
                    0.15,
                ),
                palette.status_done,
            ),
            TaskStatus::InProgress => (
                Color::from_rgba(
                    palette.status_progress.r,
                    palette.status_progress.g,
                    palette.status_progress.b,
                    0.15,
                ),
                palette.status_progress,
            ),
            TaskStatus::Blocked => (
                Color::from_rgba(
                    palette.status_blocked.r,
                    palette.status_blocked.g,
                    palette.status_blocked.b,
                    0.15,
                ),
                palette.status_blocked,
            ),
            TaskStatus::Todo => (palette.card, palette.status_todo),
            TaskStatus::Draft => (palette.card, palette.text_muted),
        };

        let is_selected = self.selected.as_ref() == Some(&node.task.id);
        let border_color = if is_selected {
            palette.accent
        } else {
            border_color
        };

        // Draw rounded rectangle
        let rect = Path::new(|builder| {
            builder.move_to(Point::new(
                pos.x - width / 2.0 + radius,
                pos.y - height / 2.0,
            ));
            builder.line_to(Point::new(
                pos.x + width / 2.0 - radius,
                pos.y - height / 2.0,
            ));
            builder.arc_to(
                Point::new(pos.x + width / 2.0, pos.y - height / 2.0),
                Point::new(pos.x + width / 2.0, pos.y - height / 2.0 + radius),
                radius,
            );
            builder.line_to(Point::new(
                pos.x + width / 2.0,
                pos.y + height / 2.0 - radius,
            ));
            builder.arc_to(
                Point::new(pos.x + width / 2.0, pos.y + height / 2.0),
                Point::new(pos.x + width / 2.0 - radius, pos.y + height / 2.0),
                radius,
            );
            builder.line_to(Point::new(
                pos.x - width / 2.0 + radius,
                pos.y + height / 2.0,
            ));
            builder.arc_to(
                Point::new(pos.x - width / 2.0, pos.y + height / 2.0),
                Point::new(pos.x - width / 2.0, pos.y + height / 2.0 - radius),
                radius,
            );
            builder.line_to(Point::new(
                pos.x - width / 2.0,
                pos.y - height / 2.0 + radius,
            ));
            builder.arc_to(
                Point::new(pos.x - width / 2.0, pos.y - height / 2.0),
                Point::new(pos.x - width / 2.0 + radius, pos.y - height / 2.0),
                radius,
            );
            builder.close();
        });

        frame.fill(&rect, bg_color);
        frame.stroke(
            &rect,
            Stroke::default().with_color(border_color).with_width(2.0),
        );

        // Draw title text
        let title = if node.task.title.len() > 20 {
            format!("{}...", &node.task.title[..17])
        } else {
            node.task.title.clone()
        };

        frame.fill_text(Text {
            content: title,
            position: Point::new(pos.x, pos.y - 8.0),
            color: palette.text,
            size: 12.0.into(),
            horizontal_alignment: iced::alignment::Horizontal::Center,
            vertical_alignment: iced::alignment::Vertical::Center,
            ..Default::default()
        });

        // Draw status label
        frame.fill_text(Text {
            content: status.as_str().to_string(),
            position: Point::new(pos.x, pos.y + 12.0),
            color: border_color,
            size: 10.0.into(),
            horizontal_alignment: iced::alignment::Horizontal::Center,
            vertical_alignment: iced::alignment::Vertical::Center,
            ..Default::default()
        });
    }

    fn draw_edge(&self, frame: &mut Frame, from: &TaskNode, to: &TaskNode, palette: &Palette) {
        let from_pos = Point::new(from.position.x, from.position.y + 25.0);
        let to_pos = Point::new(to.position.x, to.position.y - 25.0);

        // Draw line
        let line = Path::line(from_pos, to_pos);
        frame.stroke(
            &line,
            Stroke::default().with_color(palette.border).with_width(2.0),
        );

        // Draw arrow head
        let arrow_size = 8.0;
        let angle = (to_pos.y - from_pos.y).atan2(to_pos.x - from_pos.x);
        let arrow = Path::new(|builder| {
            builder.move_to(to_pos);
            builder.line_to(Point::new(
                to_pos.x - arrow_size * (angle - 0.5).cos(),
                to_pos.y - arrow_size * (angle - 0.5).sin(),
            ));
            builder.move_to(to_pos);
            builder.line_to(Point::new(
                to_pos.x - arrow_size * (angle + 0.5).cos(),
                to_pos.y - arrow_size * (angle + 0.5).sin(),
            ));
        });
        frame.stroke(
            &arrow,
            Stroke::default().with_color(palette.border).with_width(2.0),
        );
    }
}

impl<Message> canvas::Program<Message> for TaskGraph {
    type State = ();

    fn draw(
        &self,
        _state: &Self::State,
        renderer: &Renderer,
        _theme: &Theme,
        bounds: Rectangle,
        _cursor: mouse::Cursor,
    ) -> Vec<Geometry> {
        let geometry = self.cache.draw(renderer, bounds.size(), |frame| {
            let p = palette();

            // Draw edges first (below nodes)
            for &(from_idx, to_idx) in &self.edges {
                if let (Some(from), Some(to)) = (self.nodes.get(from_idx), self.nodes.get(to_idx)) {
                    self.draw_edge(frame, from, to, p);
                }
            }

            // Draw nodes
            for node in &self.nodes {
                self.draw_node(frame, node, p);
            }
        });

        vec![geometry]
    }
}

/// Helper function to create task graph element
pub fn task_graph<'a, Message: 'a>(
    tasks: &[GranaryTask],
    dependencies: &[TaskDependency],
    selected: Option<String>,
) -> Element<'a, Message> {
    let graph = TaskGraph::new(tasks, dependencies).selected(selected);

    // Calculate bounds based on node positions
    let max_level = graph.nodes.iter().map(|n| n.level).max().unwrap_or(0);
    let max_width = graph
        .nodes
        .iter()
        .map(|n| n.position.x)
        .fold(0.0f32, |a, b| a.max(b));

    let height = ((max_level + 1) * 100 + 60) as f32;
    let width = max_width + 200.0;

    Canvas::new(graph)
        .width(Length::Fixed(width.max(400.0)))
        .height(Length::Fixed(height.max(200.0)))
        .into()
}
