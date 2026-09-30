(fieldType) => {
    // Walk open shadow roots and same-origin frames. Cross-origin frames are
    // intentionally skipped by the browser's normal DOM security boundary.
    function walk(root, seen = new Set()) {
        if (!root || seen.has(root)) return [];
        seen.add(root);
        const nodes = [];
        for (const el of root.querySelectorAll ? root.querySelectorAll('*') : []) {
            nodes.push(el);
            if (el.shadowRoot) nodes.push(...walk(el.shadowRoot, seen));
            if (el.tagName === 'IFRAME') {
                try { if (el.contentDocument) nodes.push(...walk(el.contentDocument, seen)); } catch (_) {}
            }
        }
        return nodes;
    }

    function isVisible(el) {
        if (!el || !el.isConnected || el.closest('[inert]')) return false;
        const view = el.ownerDocument.defaultView || window;
        const style = view.getComputedStyle(el);
        const rect = el.getBoundingClientRect();
        return style.display !== 'none' && style.visibility !== 'hidden' &&
            style.opacity !== '0' && rect.width > 0 && rect.height > 0 &&
            (!el.checkVisibility || el.checkVisibility({checkOpacity: true, checkVisibilityCSS: true}));
    }

    function isEditable(el) {
        if (!el || el.disabled || el.readOnly || el.matches(':disabled')) return false;
        if (el.tagName === 'INPUT') {
            return fieldType === 'password'
                ? el.type === 'password'
                : ['email', 'tel', 'text', 'url', 'number'].includes(el.type);
        }
        if (fieldType === 'password') return false;
        return el.tagName === 'TEXTAREA' ||
            (el.getAttribute('contenteditable') !== 'false' &&
             (el.isContentEditable || el.getAttribute('role') === 'textbox'));
    }

    function loginHint(el) {
        return `${el.name || ''} ${el.id || ''} ${el.placeholder || ''} ` +
            `${el.getAttribute('aria-label') || ''} ${el.getAttribute('autocomplete') || ''}`;
    }

    function deepActiveElement(root) {
        let active = root && root.activeElement;
        while (active) {
            if (active.shadowRoot && active.shadowRoot.activeElement) {
                active = active.shadowRoot.activeElement;
                continue;
            }
            if (active.tagName === 'IFRAME') {
                try {
                    const inner = active.contentDocument && active.contentDocument.activeElement;
                    if (inner && inner !== active) { active = inner; continue; }
                } catch (_) {}
            }
            break;
        }
        return active;
    }

    const nodes = walk(document);
    const candidates = nodes.filter(el => isEditable(el) && isVisible(el));
    const blockedLoginField = fieldType !== 'password' && nodes.some(el => {
        if (el.tagName !== 'INPUT' || isEditable(el) || !isVisible(el)) return false;
        const hint = loginHint(el);
        return el.getAttribute('autocomplete')?.split(/\s+/).includes('username') ||
            /identifier|email|user(name)?|login/i.test(hint);
    });
    const ordered = fieldType === 'password' ? candidates : candidates.filter(el => {
        const hint = loginHint(el);
        return !/search|query|filter|find/i.test(hint);
    }).sort((a, b) => {
        const score = el => {
            const hint = loginHint(el).toLowerCase();
            if (el.tagName === 'INPUT' && el.type === 'email') return 0;
            if (el.getAttribute('autocomplete') || /identifier|email|user|login/.test(hint)) return 1;
            if (el.tagName === 'INPUT' && el.type === 'tel') return 2;
            if (el.tagName === 'INPUT' && el.type === 'text') return 3;
            return 4;
        };
        return score(a) - score(b);
    });

    const highSignal = ordered.some(el => {
        const hint = loginHint(el);
        return (el.tagName === 'INPUT' && ['email', 'tel'].includes(el.type)) ||
            el.getAttribute('autocomplete')?.split(/\s+/).includes('username') ||
            /identifier|email|user(name)?|login/i.test(hint);
    });
    if (blockedLoginField && !highSignal) return 'not_found';
    for (const el of ordered) {
        try { el.focus({preventScroll: true}); } catch (_) { el.focus(); }
        if (deepActiveElement(document) === el) return 'found';
    }
    return 'not_found';
}
