(fieldType) => {
    function usable(el) {
        if (!(el instanceof HTMLInputElement) || !el.isConnected || el.disabled ||
            el.readOnly || el.matches(':disabled') || el.closest('[inert]')) return false;
        if (fieldType === 'password' ? el.type !== 'password' : !['email', 'tel', 'text'].includes(el.type)) return false;
        const style = getComputedStyle(el);
        const rect = el.getBoundingClientRect();
        return style.display !== 'none' && style.visibility === 'visible' &&
            style.opacity !== '0' && rect.width > 0 && rect.height > 0 &&
            (!el.checkVisibility || el.checkVisibility({checkOpacity: true, checkVisibilityCSS: true}));
    }
    const selectors = fieldType === 'password' ? ['input[type="password"]'] : [
        'input[type="email"]', 'input[type="tel"]', 'input[autocomplete~="username"]',
        'input[name*="identifier" i]', 'input[name*="email" i]', 'input[name*="user" i]',
        'input[name*="login" i]', 'input[id*="identifier" i]', 'input[id*="email" i]',
        'input[id*="user" i]', 'input[id*="login" i]', 'input[type="text"]',
    ];
    for (const selector of selectors) {
        for (const el of document.querySelectorAll(selector)) {
            if (!usable(el)) continue;
            if (fieldType !== 'password' && /search|query/i.test(`${el.name} ${el.id} ${el.placeholder}`)) continue;
            el.focus();
            if (document.activeElement === el) return 'found';
        }
    }
    return 'not_found';
}
