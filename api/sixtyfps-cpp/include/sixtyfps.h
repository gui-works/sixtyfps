
#include "sixtyfps_internal.h"

namespace sixtyfps {

    // Bring opaque structure in scope
    using internal::ItemTreeNode;
    using internal::ComponentType;

    template<typename Component> void run(Component *c) {
        // FIXME! some static assert that the component is indeed a generated component matching
        // the vtable.  In fact, i think the VTable should be a static member of the Component
        internal::sixtyfps_runtime_run_component(&Component::component_type, reinterpret_cast<internal::ComponentImpl *>(c));
    }

    using internal::Rectangle;
    using internal::RectangleVTable;
    using internal::Image;
    using internal::ImageVTable;

    // the component has static lifetime so it does not need to be destroyed
    // FIXME: we probably need some kind of way to dinstinguish static component and these on the heap
    inline void dummy_destory(const ComponentType *, internal::ComponentImpl *) {}
}