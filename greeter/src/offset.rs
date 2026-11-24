// SPDX-License-Identifier: GPL-3.0-only

//! `Offset`: desenha o filho deslocado por `by` sem mudar o próprio tamanho (os vizinhos não
//! se mexem). É transparente na árvore de widgets (`tag/state/children/diff` delegam ao
//! filho, como o `Pin` do iced), então embrulhar o campo de senha não reseta foco nem texto.

use cosmic::iced::advanced::clipboard::DndDestinationRectangles;
use cosmic::iced::advanced::widget::{Id, Operation, Tree, tree};
use cosmic::iced::advanced::{Clipboard, Layout, Shell, Widget, layout, mouse, overlay, renderer};
use cosmic::iced::{Element, Event, Length, Rectangle, Size, Vector};

pub struct Offset<'a, Message, Theme, Renderer> {
    content: Element<'a, Message, Theme, Renderer>,
    by: Vector,
}

/// Embrulha `content` deslocado por `by` (pintura e área de clique; o layout dos vizinhos
/// não muda).
pub fn offset<'a, Message, Theme, Renderer>(
    content: impl Into<Element<'a, Message, Theme, Renderer>>,
    by: Vector,
) -> Offset<'a, Message, Theme, Renderer> {
    Offset {
        content: content.into(),
        by,
    }
}

impl<Message, Theme, Renderer> Widget<Message, Theme, Renderer> for Offset<'_, Message, Theme, Renderer>
where
    Renderer: cosmic::iced::advanced::Renderer,
{
    fn tag(&self) -> tree::Tag {
        self.content.as_widget().tag()
    }

    fn state(&self) -> tree::State {
        self.content.as_widget().state()
    }

    fn children(&self) -> Vec<Tree> {
        self.content.as_widget().children()
    }

    fn diff(&mut self, tree: &mut Tree) {
        self.content.as_widget_mut().diff(tree);
    }

    fn size(&self) -> Size<Length> {
        self.content.as_widget().size()
    }

    fn size_hint(&self) -> Size<Length> {
        self.content.as_widget().size_hint()
    }

    fn layout(&mut self, tree: &mut Tree, renderer: &Renderer, limits: &layout::Limits) -> layout::Node {
        let child = self.content.as_widget_mut().layout(tree, renderer, limits);
        let size = child.size();
        layout::Node::with_children(size, vec![child.translate(self.by)])
    }

    fn operate(
        &mut self,
        tree: &mut Tree,
        layout: Layout<'_>,
        renderer: &Renderer,
        operation: &mut dyn Operation,
    ) {
        if let Some(child) = layout.children().next() {
            self.content.as_widget_mut().operate(tree, child, renderer, operation);
        }
    }

    fn update(
        &mut self,
        tree: &mut Tree,
        event: &Event,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        renderer: &Renderer,
        clipboard: &mut dyn Clipboard,
        shell: &mut Shell<'_, Message>,
        viewport: &Rectangle,
    ) {
        if let Some(child) = layout.children().next() {
            self.content
                .as_widget_mut()
                .update(tree, event, child, cursor, renderer, clipboard, shell, viewport);
        }
    }

    fn mouse_interaction(
        &self,
        tree: &Tree,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
        renderer: &Renderer,
    ) -> mouse::Interaction {
        layout.children().next().map_or(mouse::Interaction::None, |child| {
            self.content
                .as_widget()
                .mouse_interaction(tree, child, cursor, viewport, renderer)
        })
    }

    /// Passa o `viewport` recebido sem recortar pelos próprios limites (o `Pin` recortaria o
    /// texto deslocado para fora do nó).
    fn draw(
        &self,
        tree: &Tree,
        renderer: &mut Renderer,
        theme: &Theme,
        style: &renderer::Style,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
    ) {
        if let Some(child) = layout.children().next() {
            self.content
                .as_widget()
                .draw(tree, renderer, theme, style, child, cursor, viewport);
        }
    }

    fn overlay<'b>(
        &'b mut self,
        tree: &'b mut Tree,
        layout: Layout<'b>,
        renderer: &Renderer,
        viewport: &Rectangle,
        translation: Vector,
    ) -> Option<overlay::Element<'b, Message, Theme, Renderer>> {
        let child = layout.children().next()?;
        self.content
            .as_widget_mut()
            .overlay(tree, child, renderer, viewport, translation)
    }

    fn id(&self) -> Option<Id> {
        self.content.as_widget().id()
    }

    fn set_id(&mut self, id: Id) {
        self.content.as_widget_mut().set_id(id);
    }

    fn drag_destinations(
        &self,
        state: &Tree,
        layout: Layout<'_>,
        renderer: &Renderer,
        dnd_rectangles: &mut DndDestinationRectangles,
    ) {
        if let Some(child) = layout.children().next() {
            self.content.as_widget().drag_destinations(
                state,
                child.with_virtual_offset(layout.virtual_offset()),
                renderer,
                dnd_rectangles,
            );
        }
    }
}

impl<'a, Message, Theme, Renderer> From<Offset<'a, Message, Theme, Renderer>>
    for Element<'a, Message, Theme, Renderer>
where
    Message: 'a,
    Theme: 'a,
    Renderer: cosmic::iced::advanced::Renderer + 'a,
{
    fn from(offset: Offset<'a, Message, Theme, Renderer>) -> Self {
        Element::new(offset)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cosmic::iced::advanced::layout::Limits;
    use cosmic::iced::advanced::widget::Tree;
    use cosmic::iced::widget::Space;

    fn space() -> Space { Space::new().width(10.0).height(20.0) }

    #[test]
    fn offset_moves_child_but_keeps_own_size() {
        let mut w: Offset<'_, (), (), ()> = offset(space(), Vector::new(3.0, 5.0));
        let mut tree = Tree::new(&w as &dyn Widget<(), (), ()>);
        let node = w.layout(&mut tree, &(), &Limits::new(Size::ZERO, Size::new(100.0, 100.0)));
        assert_eq!(node.size(), Size::new(10.0, 20.0));
        assert_eq!(node.children()[0].bounds(), Rectangle::new([3.0, 5.0].into(), Size::new(10.0, 20.0)));
    }
    #[test]
    fn offset_tree_is_transparent() {
        let w: Offset<'_, (), (), ()> = offset(space(), Vector::new(0.0, 12.0));
        let a = Tree::new(&w as &dyn Widget<(), (), ()>);
        let b = Tree::new(&space() as &dyn Widget<(), (), ()>);
        assert_eq!(a.tag, b.tag);
        assert_eq!(a.children.len(), b.children.len());
    }
    /// Com um filho com estado (o `Space` não tem), a árvore do `Offset` tem de ser a do
    /// próprio filho: mesma tag não vazia e o mesmo filho único.
    #[test]
    fn offset_tree_is_transparent_for_stateful_child() {
        use cosmic::iced::advanced::widget::tree::Tag;
        use cosmic::iced::widget::{MouseArea, mouse_area};
        let w: Offset<'_, (), (), ()> = offset(mouse_area(space()), Vector::new(0.0, 12.0));
        let inner: MouseArea<'_, (), (), ()> = mouse_area(space());
        let a = Tree::new(&w as &dyn Widget<(), (), ()>);
        let b = Tree::new(&inner as &dyn Widget<(), (), ()>);
        assert_ne!(b.tag, Tag::stateless());
        assert_eq!(a.tag, b.tag);
        assert_eq!(a.children.len(), 1);
        assert_eq!(a.children.len(), b.children.len());
        assert_eq!(a.children[0].tag, b.children[0].tag);
    }
}
