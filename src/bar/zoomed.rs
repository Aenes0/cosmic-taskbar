// SPDX-License-Identifier: GPL-3.0-only

//! An icon that draws itself larger, and a little higher, than the room it
//! takes: its layout, its hit area and its events are its content's, only its
//! drawing is transformed. The panel is a fixed-height surface with no room
//! to grow an icon in the layout, and the neighbours must not move.

use cosmic::iced::advanced::{
    Clipboard, Layout, Shell, Widget, layout, mouse, overlay, renderer,
    widget::{self, tree},
};
use cosmic::iced::advanced::Renderer as _;
use cosmic::iced::core::{Transformation, clipboard::DndDestinationRectangles};
use cosmic::iced::{Element, Event, Length, Rectangle, Size, Vector};

/// Where the icon is drawn, given how much it has grown and how high it is lifted.
pub struct Zoomed<'a, Message> {
    content: Element<'a, Message, cosmic::Theme, cosmic::Renderer>,
    scale: f32,
    /// Px upward, on top of the growth.
    rise: f32,
}

impl<'a, Message> Zoomed<'a, Message> {
    pub fn new(
        content: impl Into<Element<'a, Message, cosmic::Theme, cosmic::Renderer>>,
        scale: f32,
        rise: f32,
    ) -> Self {
        Self { content: content.into(), scale, rise }
    }

    fn at_rest(&self) -> bool {
        (self.scale - 1.0).abs() < 0.002 && self.rise.abs() < 0.05
    }
}

/// The transformation of an icon whose box has centre `(cx, cy)` and half
/// height `half`: grown about a point a little above the box's middle, so
/// it grows more upwards than down (the line under it stays clear), and
/// lifted by `rise`.
pub fn icon_transformation(cx: f32, cy: f32, half: f32, scale: f32, rise: f32) -> Transformation {
    let anchor_y = cy + half * 0.14;
    Transformation::translate(cx, anchor_y - rise)
        * Transformation::scale(scale)
        * Transformation::translate(-cx, -anchor_y)
}

impl<Message> Widget<Message, cosmic::Theme, cosmic::Renderer> for Zoomed<'_, Message> {
    fn tag(&self) -> tree::Tag {
        self.content.as_widget().tag()
    }

    fn state(&self) -> tree::State {
        self.content.as_widget().state()
    }

    fn children(&self) -> Vec<tree::Tree> {
        self.content.as_widget().children()
    }

    fn diff(&mut self, tree: &mut widget::Tree) {
        self.content.as_widget_mut().diff(tree);
    }

    fn size(&self) -> Size<Length> {
        self.content.as_widget().size()
    }

    fn size_hint(&self) -> Size<Length> {
        self.content.as_widget().size_hint()
    }

    fn layout(
        &mut self,
        tree: &mut widget::Tree,
        renderer: &cosmic::Renderer,
        limits: &layout::Limits,
    ) -> layout::Node {
        self.content.as_widget_mut().layout(tree, renderer, limits)
    }

    fn update(
        &mut self,
        tree: &mut widget::Tree,
        event: &Event,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        renderer: &cosmic::Renderer,
        clipboard: &mut dyn Clipboard,
        shell: &mut Shell<'_, Message>,
        viewport: &Rectangle,
    ) {
        self.content
            .as_widget_mut()
            .update(tree, event, layout, cursor, renderer, clipboard, shell, viewport);
    }

    fn draw(
        &self,
        tree: &widget::Tree,
        renderer: &mut cosmic::Renderer,
        theme: &cosmic::Theme,
        style: &renderer::Style,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
    ) {
        if self.at_rest() {
            self.content
                .as_widget()
                .draw(tree, renderer, theme, style, layout, cursor, viewport);
            return;
        }
        let bounds = layout.bounds();
        let transformation = icon_transformation(
            bounds.x + bounds.width / 2.0,
            bounds.y + bounds.height / 2.0,
            bounds.height / 2.0,
            self.scale,
            self.rise,
        );
        renderer.with_transformation(transformation, |renderer| {
            self.content
                .as_widget()
                .draw(tree, renderer, theme, style, layout, cursor, viewport);
        });
    }

    fn mouse_interaction(
        &self,
        tree: &widget::Tree,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
        renderer: &cosmic::Renderer,
    ) -> mouse::Interaction {
        self.content
            .as_widget()
            .mouse_interaction(tree, layout, cursor, viewport, renderer)
    }

    fn operate(
        &mut self,
        tree: &mut widget::Tree,
        layout: Layout<'_>,
        renderer: &cosmic::Renderer,
        operation: &mut dyn widget::Operation,
    ) {
        self.content
            .as_widget_mut()
            .operate(tree, layout, renderer, operation);
    }

    fn overlay<'b>(
        &'b mut self,
        state: &'b mut widget::Tree,
        layout: Layout<'b>,
        renderer: &cosmic::Renderer,
        viewport: &Rectangle,
        offset: Vector,
    ) -> Option<overlay::Element<'b, Message, cosmic::Theme, cosmic::Renderer>> {
        self.content
            .as_widget_mut()
            .overlay(state, layout, renderer, viewport, offset)
    }

    fn drag_destinations(
        &self,
        state: &widget::Tree,
        layout: Layout<'_>,
        renderer: &cosmic::Renderer,
        dnd_rectangles: &mut DndDestinationRectangles,
    ) {
        self.content
            .as_widget()
            .drag_destinations(state, layout, renderer, dnd_rectangles);
    }
}

impl<'a, Message: 'a> From<Zoomed<'a, Message>> for Element<'a, Message, cosmic::Theme, cosmic::Renderer> {
    fn from(zoomed: Zoomed<'a, Message>) -> Self {
        Element::new(zoomed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn point(t: Transformation, x: f32, y: f32) -> (f32, f32) {
        let r = Rectangle::new(cosmic::iced::Point::new(x, y), Size::new(0.0, 0.0)) * t;
        (r.x, r.y)
    }

    #[test]
    fn at_scale_one_and_no_rise_nothing_moves() {
        let t = icon_transformation(24.0, 20.0, 16.0, 1.0, 0.0);
        let (x, y) = point(t, 10.0, 12.0);
        assert!((x - 10.0).abs() < 0.001 && (y - 12.0).abs() < 0.001, "{x} {y}");
    }

    #[test]
    fn it_grows_more_upwards_than_downwards() {
        let t = icon_transformation(24.0, 20.0, 16.0, 1.25, 0.0);
        let (_, top) = point(t, 24.0, 4.0);
        let (_, bottom) = point(t, 24.0, 36.0);
        let grown_up = 4.0 - top;
        let grown_down = bottom - 36.0;
        assert!(grown_up > grown_down, "up {grown_up} down {grown_down}");
        assert!(grown_up > 0.0 && grown_down >= 0.0);
        // The whole box grew by 25%: 32 px to 40 px.
        assert!((bottom - top - 40.0).abs() < 0.01, "{}", bottom - top);
    }

    #[test]
    fn the_rise_lifts_the_icon() {
        let t = icon_transformation(24.0, 20.0, 16.0, 1.0, 4.0);
        let (_, y) = point(t, 24.0, 20.0);
        assert!((y - 16.0).abs() < 0.001, "{y}");
    }
}
