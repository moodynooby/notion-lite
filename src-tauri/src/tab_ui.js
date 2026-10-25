(function() {
    const currentTabId = '__TAB_ID__';
    const currentWindowId = '__WINDOW_ID__';
    const initialTheme = '__THEME__';

    if (window.__NOTION_LITE_INITIALIZED__) {
        return;
    }
    window.__NOTION_LITE_INITIALIZED__ = true;

    // Robust Tauri IPC Invoker supporting Tauri v2 namespaces and internal bridges
    const safeInvoke = async (cmd, args = {}) => {
        if (window.__TAURI__ && window.__TAURI__.core && typeof window.__TAURI__.core.invoke === 'function') {
            try {
                return await window.__TAURI__.core.invoke(cmd, args);
            } catch (e) {
                console.warn('[NotionLite IPC] __TAURI__.core.invoke error for ' + cmd + ':', e);
                throw e;
            }
        }
        if (window.__TAURI_INTERNALS__ && typeof window.__TAURI_INTERNALS__.invoke === 'function') {
            try {
                return await window.__TAURI_INTERNALS__.invoke(cmd, args);
            } catch (e) {
                console.warn('[NotionLite IPC] __TAURI_INTERNALS__.invoke error for ' + cmd + ':', e);
                throw e;
            }
        }
        if (window.__TAURI__ && typeof window.__TAURI__.invoke === 'function') {
            try {
                return await window.__TAURI__.invoke(cmd, args);
            } catch (e) {
                console.warn('[NotionLite IPC] __TAURI__.invoke error for ' + cmd + ':', e);
                throw e;
            }
        }
        throw new Error('Tauri IPC is not available in current context');
    };

    // 1. Debounced Title Observer for syncing tab title
    let lastTitle = document.title;
    let titleDebounceTimer = null;
    const syncTitle = () => {
        if (document.title && document.title !== lastTitle) {
            lastTitle = document.title;
            safeInvoke('update_tab_state', {
                tabId: currentTabId,
                title: lastTitle,
                url: window.location.href
            }).catch(() => {});
            if (window.NotionLite && window.NotionLite.refreshTabs) {
                window.NotionLite.refreshTabs();
            }
        }
    };
    const titleObserver = new MutationObserver(function() {
        if (document.title !== lastTitle) {
            if (titleDebounceTimer) clearTimeout(titleDebounceTimer);
            titleDebounceTimer = setTimeout(syncTitle, 200);
        }
    });
    const titleNode = document.querySelector('title');
    if (titleNode) {
        titleObserver.observe(titleNode, { subtree: true, characterData: true, childList: true });
    } else if (document.head) {
        titleObserver.observe(document.head, { childList: true });
    }

    function runInit() {
        if (!document.body || !document.head) {
            document.addEventListener('DOMContentLoaded', runInit);
            return;
        }
        if (document.getElementById('notion-lite-header')) return;

        document.documentElement.setAttribute('data-nl-theme', initialTheme);

        // 2. CSS Styles: Notion desktop app-like header and root margin fix
        const styleEl = document.createElement('style');
        styleEl.id = 'notion-lite-injected-style';
        styleEl.textContent = `
            html, body {
                margin: 0 !important;
                padding: 0 !important;
                overflow: hidden !important;
                -webkit-font-smoothing: antialiased !important;
                -moz-osx-font-smoothing: grayscale !important;
                text-rendering: optimizeLegibility !important;
            }

            /* Notion layout fix: Push the root container down so page titles, covers & breadcrumbs are never cut off */
            #notion-app {
                margin-top: 38px !important;
                height: calc(100vh - 38px) !important;
                box-sizing: border-box !important;
            }

            .notion-overlay-container {
                top: 38px !important;
                height: calc(100vh - 38px) !important;
            }

            #notion-lite-header {
                position: fixed;
                top: 0;
                left: 0;
                width: 100%;
                height: 38px;
                z-index: 999999;
                display: flex;
                align-items: center;
                justify-content: space-between;
                box-sizing: border-box;
                padding: 0 8px;
                background: #f7f6f3;
                border-bottom: 1px solid rgba(0, 0, 0, 0.08);
                user-select: none;
                -webkit-user-select: none;
                -webkit-app-region: drag;
                font-family: system-ui, -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, Oxygen, Ubuntu, Cantarell, "Noto Sans", "Liberation Sans", sans-serif, "Apple Color Emoji", "Segoe UI Emoji", "Noto Color Emoji";
                font-size: 13px;
                color: #37352f;
                -webkit-font-smoothing: antialiased;
                -moz-osx-font-smoothing: grayscale;
                text-rendering: optimizeLegibility;
            }
            [data-nl-theme="dracula"] #notion-lite-header {
                background: #191a21;
                border-bottom: 1px solid #383a59;
                color: #f8f8f2;
            }
            [data-nl-theme="nord"] #notion-lite-header {
                background: #242933;
                border-bottom: 1px solid #3b4252;
                color: #eceff4;
            }
            [data-nl-theme="default"] #notion-lite-header {
                background: #f7f6f3;
                border-bottom: 1px solid rgba(0, 0, 0, 0.08);
                color: #37352f;
            }

            .nl-nav-group {
                display: flex;
                align-items: center;
                gap: 2px;
                flex-shrink: 0;
                margin-right: 8px;
                -webkit-app-region: no-drag;
            }
            .nl-btn {
                background: transparent;
                border: none;
                outline: none;
                color: inherit;
                cursor: pointer;
                width: 26px;
                height: 26px;
                padding: 0;
                border-radius: 4px;
                display: flex;
                align-items: center;
                justify-content: center;
                transition: background 0.15s, opacity 0.15s;
                opacity: 0.7;
                -webkit-app-region: no-drag;
            }
            .nl-btn:hover {
                opacity: 1;
                background: rgba(0, 0, 0, 0.06);
            }
            [data-nl-theme="dracula"] .nl-btn:hover,
            [data-nl-theme="nord"] .nl-btn:hover {
                background: rgba(255, 255, 255, 0.1);
            }
            .nl-btn:active {
                transform: scale(0.96);
            }

            .nl-tabs-strip {
                display: flex;
                align-items: flex-end;
                gap: 3px;
                flex-grow: 1;
                height: 100%;
                overflow-x: auto;
                overflow-y: hidden;
                scrollbar-width: none;
                padding-top: 5px;
                box-sizing: border-box;
                -webkit-app-region: drag;
            }
            .nl-tabs-strip::-webkit-scrollbar {
                display: none;
            }
            .nl-tab {
                display: flex;
                align-items: center;
                gap: 6px;
                padding: 0 10px;
                height: 29px;
                box-sizing: border-box;
                border-radius: 6px 6px 0 0;
                background: rgba(0, 0, 0, 0.04);
                color: rgba(55, 53, 47, 0.75);
                cursor: pointer;
                font-family: system-ui, -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, Oxygen, Ubuntu, Cantarell, "Noto Sans", "Liberation Sans", sans-serif;
                font-size: 12px;
                font-weight: 500;
                max-width: 180px;
                min-width: 85px;
                transition: background 0.12s, color 0.12s;
                border: 1px solid transparent;
                border-bottom: none;
                -webkit-app-region: no-drag;
                -webkit-font-smoothing: antialiased;
                -moz-osx-font-smoothing: grayscale;
            }
            [data-nl-theme="dracula"] .nl-tab,
            [data-nl-theme="nord"] .nl-tab {
                background: rgba(255, 255, 255, 0.05);
                color: rgba(255, 255, 255, 0.65);
            }
            .nl-tab:hover {
                background: rgba(0, 0, 0, 0.08);
                color: #000000;
            }
            [data-nl-theme="dracula"] .nl-tab:hover,
            [data-nl-theme="nord"] .nl-tab:hover {
                background: rgba(255, 255, 255, 0.1);
                color: #ffffff;
            }
            .nl-tab.active {
                background: #ffffff;
                color: #37352f;
                font-weight: 500;
                border: 1px solid rgba(0, 0, 0, 0.08);
                border-bottom: 1px solid #ffffff;
                box-shadow: 0 -1px 3px rgba(0, 0, 0, 0.03);
            }
            [data-nl-theme="dracula"] .nl-tab.active {
                background: #282a36;
                color: #f8f8f2;
                border: 1px solid #44475a;
                border-bottom: 1px solid #282a36;
                box-shadow: none;
            }
            [data-nl-theme="nord"] .nl-tab.active {
                background: #2e3440;
                color: #eceff4;
                border: 1px solid #3b4252;
                border-bottom: 1px solid #2e3440;
                box-shadow: none;
            }
            .nl-tab-icon {
                display: flex;
                align-items: center;
                justify-content: center;
                flex-shrink: 0;
                opacity: 0.8;
            }
            .nl-tab-title {
                white-space: nowrap;
                overflow: hidden;
                text-overflow: ellipsis;
                flex-grow: 1;
            }
            .nl-tab-close {
                display: flex;
                align-items: center;
                justify-content: center;
                width: 16px;
                height: 16px;
                border-radius: 3px;
                opacity: 0.45;
                font-size: 13px;
                line-height: 1;
                transition: opacity 0.15s, background 0.15s;
                flex-shrink: 0;
            }
            .nl-tab-close:hover {
                opacity: 1;
                background: rgba(0, 0, 0, 0.1);
            }
            [data-nl-theme="dracula"] .nl-tab-close:hover,
            [data-nl-theme="nord"] .nl-tab-close:hover {
                background: rgba(255, 255, 255, 0.2);
            }
            .nl-new-tab-btn {
                height: 26px;
                width: 26px;
                margin-bottom: 2px;
                font-size: 15px;
                font-weight: 500;
                border-radius: 4px;
                display: flex;
                align-items: center;
                justify-content: center;
                flex-shrink: 0;
                -webkit-app-region: no-drag;
            }

            .nl-actions-group {
                display: flex;
                align-items: center;
                gap: 4px;
                flex-shrink: 0;
                margin-left: 8px;
                -webkit-app-region: no-drag;
            }

            .nl-window-controls {
                display: flex;
                align-items: center;
                gap: 2px;
                margin-left: 6px;
                padding-left: 6px;
                border-left: 1px solid rgba(0, 0, 0, 0.08);
                -webkit-app-region: no-drag;
            }
            [data-nl-theme="dracula"] .nl-window-controls,
            [data-nl-theme="nord"] .nl-window-controls {
                border-left: 1px solid rgba(255, 255, 255, 0.1);
            }
            .nl-win-btn {
                width: 26px;
                height: 26px;
                padding: 0;
                display: flex;
                align-items: center;
                justify-content: center;
                border-radius: 4px;
                opacity: 0.7;
                -webkit-app-region: no-drag;
            }
            .nl-win-btn:hover {
                opacity: 1;
            }
            .nl-win-close:hover {
                background: #e81123 !important;
                color: #ffffff !important;
                opacity: 1 !important;
            }

            /* Preferences Modal Overlay */
            .nl-modal-backdrop {
                position: fixed;
                top: 0;
                left: 0;
                width: 100vw;
                height: 100vh;
                z-index: 10000000;
                background: rgba(0, 0, 0, 0.65);
                backdrop-filter: blur(5px);
                display: flex;
                align-items: center;
                justify-content: center;
                animation: nl-fade-in 0.15s ease-out;
                user-select: none;
                -webkit-user-select: none;
            }
            @keyframes nl-fade-in {
                from { opacity: 0; }
                to { opacity: 1; }
            }
            .nl-modal-card {
                width: 500px;
                max-width: 92vw;
                background: #282a36;
                color: #f8f8f2;
                border: 1px solid #44475a;
                border-radius: 10px;
                box-shadow: 0 20px 45px rgba(0, 0, 0, 0.5);
                display: flex;
                flex-direction: column;
                overflow: hidden;
                font-family: system-ui, -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, Oxygen, Ubuntu, Cantarell, "Noto Sans", "Liberation Sans", sans-serif;
                -webkit-font-smoothing: antialiased;
                -moz-osx-font-smoothing: grayscale;
            }
            [data-nl-theme="nord"] .nl-modal-card { background: #2e3440; color: #eceff4; border-color: #3b4252; }
            [data-nl-theme="default"] .nl-modal-card { background: #ffffff; color: #37352f; border-color: #e0e0e0; box-shadow: 0 15px 35px rgba(0, 0, 0, 0.2); }

            .nl-modal-header {
                display: flex;
                align-items: center;
                justify-content: space-between;
                padding: 14px 18px;
                border-bottom: 1px solid rgba(255, 255, 255, 0.08);
            }
            [data-nl-theme="default"] .nl-modal-header { border-bottom: 1px solid rgba(0, 0, 0, 0.08); }
            .nl-modal-title {
                display: flex;
                align-items: center;
                gap: 8px;
                font-size: 15px;
                font-weight: 600;
            }
            .nl-modal-close {
                background: transparent;
                border: none;
                color: inherit;
                cursor: pointer;
                font-size: 18px;
                opacity: 0.6;
                line-height: 1;
                padding: 4px;
                border-radius: 4px;
            }
            .nl-modal-close:hover { opacity: 1; background: rgba(255, 255, 255, 0.1); }
            [data-nl-theme="default"] .nl-modal-close:hover { background: rgba(0, 0, 0, 0.06); }

            .nl-modal-body {
                padding: 18px;
                display: flex;
                flex-direction: column;
                gap: 16px;
                max-height: 70vh;
                overflow-y: auto;
            }
            .nl-section-title {
                font-size: 11px;
                font-weight: 600;
                text-transform: uppercase;
                letter-spacing: 0.5px;
                opacity: 0.6;
                margin-bottom: 8px;
            }
            .nl-theme-grid {
                display: grid;
                grid-template-columns: repeat(3, 1fr);
                gap: 8px;
            }
            .nl-theme-card {
                border: 2px solid rgba(255, 255, 255, 0.1);
                border-radius: 6px;
                padding: 10px;
                cursor: pointer;
                text-align: center;
                font-size: 12px;
                font-weight: 500;
                transition: border-color 0.15s, background 0.15s;
            }
            [data-nl-theme="default"] .nl-theme-card { border-color: rgba(0, 0, 0, 0.1); }
            .nl-theme-card:hover { border-color: #bd93f9; }
            .nl-theme-card.selected {
                border-color: #bd93f9;
                background: rgba(189, 147, 249, 0.15);
            }
            [data-nl-theme="default"] .nl-theme-card.selected {
                border-color: #2383e2;
                background: rgba(35, 131, 226, 0.1);
            }
            .nl-field-row {
                display: flex;
                align-items: center;
                justify-content: space-between;
                padding: 8px 0;
                border-bottom: 1px solid rgba(255, 255, 255, 0.05);
            }
            [data-nl-theme="default"] .nl-field-row { border-bottom: 1px solid rgba(0, 0, 0, 0.05); }
            .nl-field-label {
                font-size: 13px;
                font-weight: 500;
            }
            .nl-field-desc {
                font-size: 11px;
                opacity: 0.6;
                margin-top: 2px;
            }
            .nl-input {
                width: 100%;
                padding: 8px 10px;
                box-sizing: border-box;
                background: rgba(0, 0, 0, 0.2);
                border: 1px solid rgba(255, 255, 255, 0.15);
                border-radius: 6px;
                color: inherit;
                font-size: 13px;
                margin-top: 6px;
                outline: none;
            }
            [data-nl-theme="default"] .nl-input {
                background: rgba(0, 0, 0, 0.03);
                border-color: rgba(0, 0, 0, 0.15);
            }
            .nl-input:focus { border-color: #bd93f9; }
            [data-nl-theme="default"] .nl-input:focus { border-color: #2383e2; }

            .nl-switch {
                position: relative;
                display: inline-block;
                width: 38px;
                height: 22px;
            }
            .nl-switch input { opacity: 0; width: 0; height: 0; }
            .nl-slider {
                position: absolute;
                cursor: pointer;
                top: 0; left: 0; right: 0; bottom: 0;
                background-color: rgba(255, 255, 255, 0.2);
                transition: 0.2s;
                border-radius: 22px;
            }
            [data-nl-theme="default"] .nl-slider { background-color: rgba(0, 0, 0, 0.15); }
            .nl-slider:before {
                position: absolute;
                content: "";
                height: 16px;
                width: 16px;
                left: 3px;
                bottom: 3px;
                background-color: white;
                transition: 0.2s;
                border-radius: 50%;
            }
            .nl-switch input:checked + .nl-slider { background-color: #50fa7b; }
            [data-nl-theme="default"] .nl-switch input:checked + .nl-slider { background-color: #2eaadc; }
            .nl-switch input:checked + .nl-slider:before { transform: translateX(16px); }

            .nl-btn-row {
                display: flex;
                gap: 8px;
                margin-top: 6px;
            }
            .nl-action-btn {
                padding: 6px 12px;
                border-radius: 5px;
                font-size: 12px;
                font-weight: 500;
                cursor: pointer;
                border: none;
                transition: background 0.15s, opacity 0.15s;
            }
            .nl-btn-sec {
                background: rgba(255, 255, 255, 0.1);
                color: inherit;
            }
            [data-nl-theme="default"] .nl-btn-sec { background: rgba(0, 0, 0, 0.06); }
            .nl-btn-sec:hover { background: rgba(255, 255, 255, 0.18); }
            [data-nl-theme="default"] .nl-btn-sec:hover { background: rgba(0, 0, 0, 0.1); }

            .nl-btn-pri {
                background: #bd93f9;
                color: #282a36;
                font-weight: 600;
            }
            [data-nl-theme="default"] .nl-btn-pri { background: #2383e2; color: #ffffff; }
            .nl-btn-pri:hover { opacity: 0.9; }

            .nl-status-text {
                font-size: 11px;
                margin-top: 6px;
                opacity: 0.8;
                min-height: 14px;
            }
            .nl-modal-footer {
                display: flex;
                align-items: center;
                justify-content: space-between;
                padding: 12px 18px;
                border-top: 1px solid rgba(255, 255, 255, 0.08);
                background: rgba(0, 0, 0, 0.1);
            }
            [data-nl-theme="default"] .nl-modal-footer { border-top: 1px solid rgba(0, 0, 0, 0.08); background: rgba(0, 0, 0, 0.02); }
        `;
        document.head.appendChild(styleEl);

        // 3. Create Header DOM
        const header = document.createElement('div');
        header.id = 'notion-lite-header';
        header.setAttribute('data-tauri-drag-region', '');

        // Left navigation group
        const navGroup = document.createElement('div');
        navGroup.className = 'nl-nav-group';

        const backBtn = document.createElement('button');
        backBtn.className = 'nl-btn';
        backBtn.title = 'Back (Alt+Left)';
        backBtn.innerHTML = '<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.2" stroke-linecap="round" stroke-linejoin="round"><path d="M15 18l-6-6 6-6"/></svg>';
        backBtn.onclick = (e) => { e.stopPropagation(); window.history.back(); };

        const fwdBtn = document.createElement('button');
        fwdBtn.className = 'nl-btn';
        fwdBtn.title = 'Forward (Alt+Right)';
        fwdBtn.innerHTML = '<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.2" stroke-linecap="round" stroke-linejoin="round"><path d="M9 18l6-6-6-6"/></svg>';
        fwdBtn.onclick = (e) => { e.stopPropagation(); window.history.forward(); };

        const reloadBtn = document.createElement('button');
        reloadBtn.className = 'nl-btn';
        reloadBtn.title = 'Reload (F5 / Ctrl+R)';
        reloadBtn.innerHTML = '<svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.2" stroke-linecap="round" stroke-linejoin="round"><path d="M23 4v6h-6M1 20v-6h6"/><path d="M3.51 9a9 9 0 0 1 14.85-3.36L23 10M1 14l4.64 4.36A9 9 0 0 0 20.49 15"/></svg>';
        reloadBtn.onclick = (e) => { e.stopPropagation(); window.location.reload(); };

        navGroup.appendChild(backBtn);
        navGroup.appendChild(fwdBtn);
        navGroup.appendChild(reloadBtn);

        // Center tab strip
        const tabsStrip = document.createElement('div');
        tabsStrip.className = 'nl-tabs-strip';
        tabsStrip.setAttribute('data-tauri-drag-region', '');

        let lastTabsDigest = '';
        const updateTabsDom = (tabs) => {
            const currentDigest = tabs.map(t => `${t.id}:${t.title}:${t.id === currentTabId}`).join('|');
            if (currentDigest === lastTabsDigest && tabsStrip.children.length > 0) {
                return; // Tabs haven't changed, skip DOM reconstruction
            }
            lastTabsDigest = currentDigest;
            tabsStrip.innerHTML = '';
            tabs.forEach(t => {
                const tabEl = document.createElement('div');
                tabEl.className = 'nl-tab' + (t.id === currentTabId ? ' active' : '');
                const cleanTitle = (t.title && t.title.trim().length > 0) ? t.title.trim() : 'Notion';
                tabEl.title = cleanTitle;

                // Notion page icon placeholder
                const iconSpan = document.createElement('span');
                iconSpan.className = 'nl-tab-icon';
                iconSpan.innerHTML = '<svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z"/><polyline points="14 2 14 8 20 8"/><line x1="16" y1="13" x2="8" y2="13"/><line x1="16" y1="17" x2="8" y2="17"/></svg>';

                const titleSpan = document.createElement('span');
                titleSpan.className = 'nl-tab-title';
                titleSpan.textContent = cleanTitle;

                const closeSpan = document.createElement('span');
                closeSpan.className = 'nl-tab-close';
                closeSpan.textContent = '×';
                closeSpan.title = 'Close Tab';
                closeSpan.onclick = async (e) => {
                    e.stopPropagation();
                    try {
                        await safeInvoke('close_tab', { tabId: t.id });
                        setTimeout(renderTabs, 50);
                    } catch (err) {
                        console.warn('[NotionLite] close_tab fallback:', err);
                        window.location.href = 'notion-lite-action://tab:close:' + t.id;
                    }
                };

                tabEl.appendChild(iconSpan);
                tabEl.appendChild(titleSpan);
                tabEl.appendChild(closeSpan);

                tabEl.onclick = async () => {
                    if (t.id !== currentTabId) {
                        try {
                            await safeInvoke('switch_tab', { tabId: t.id });
                        } catch (err) {
                            console.warn('[NotionLite] switch_tab fallback:', err);
                            window.location.href = 'notion-lite-action://tab:switch:' + t.id;
                        }
                    }
                };
                tabsStrip.appendChild(tabEl);
            });

            const newTabBtn = document.createElement('button');
            newTabBtn.className = 'nl-btn nl-new-tab-btn';
            newTabBtn.title = 'New Tab';
            newTabBtn.textContent = '+';
            newTabBtn.onclick = async (e) => {
                e.stopPropagation();
                try {
                    await safeInvoke('new_tab', { windowId: currentWindowId });
                    setTimeout(renderTabs, 50);
                    setTimeout(renderTabs, 300);
                } catch (err) {
                    console.warn('[NotionLite] new_tab fallback:', err);
                    window.location.href = 'notion-lite-action://tab:new';
                }
            };
            tabsStrip.appendChild(newTabBtn);
        };

        const renderTabs = async () => {
            try {
                const tabs = await safeInvoke('get_window_tabs', { windowId: currentWindowId });
                if (Array.isArray(tabs) && tabs.length > 0) {
                    updateTabsDom(tabs);
                    return;
                }
            } catch (err) {
                // Fallback: render at least active tab with current document.title
            }
            updateTabsDom([{ id: currentTabId, title: document.title || 'Notion' }]);
        };

        // Right actions group
        const actionsGroup = document.createElement('div');
        actionsGroup.className = 'nl-actions-group';

        const prefsBtn = document.createElement('button');
        prefsBtn.className = 'nl-btn';
        prefsBtn.title = 'Preferences (Ctrl+,)';
        prefsBtn.innerHTML = '<svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><circle cx="12" cy="12" r="3"/><path d="M19.4 15a1.65 1.65 0 0 0 .33 1.82l.06.06a2 2 0 0 1 0 2.83 2 2 0 0 1-2.83 0l-.06-.06a1.65 1.65 0 0 0-1.82-.33 1.65 1.65 0 0 0-1 1.51V21a2 2 0 0 1-2 2 2 2 0 0 1-2-2v-.09A1.65 1.65 0 0 0 9 19.4a1.65 1.65 0 0 0-1.82.33l-.06.06a2 2 0 0 1-2.83 0 2 2 0 0 1 0-2.83l.06-.06a1.65 1.65 0 0 0 .33-1.82 1.65 1.65 0 0 0-1.51-1H3a2 2 0 0 1-2-2 2 2 0 0 1 2-2h.09A1.65 1.65 0 0 0 4.6 9a1.65 1.65 0 0 0-.33-1.82l-.06-.06a2 2 0 0 1 0-2.83 2 2 0 0 1 2.83 0l.06.06a1.65 1.65 0 0 0 1.82.33H9a1.65 1.65 0 0 0 1-1.51V3a2 2 0 0 1 2-2 2 2 0 0 1 2 2v.09a1.65 1.65 0 0 0 1 1.51 1.65 1.65 0 0 0 1.82-.33l.06-.06a2 2 0 0 1 2.83 0 2 2 0 0 1 0 2.83l-.06.06a1.65 1.65 0 0 0-.33 1.82V9a1.65 1.65 0 0 0 1.51 1H21a2 2 0 0 1 2 2 2 2 0 0 1-2 2h-.09a1.65 1.65 0 0 0-1.51 1z"/></svg>';
        prefsBtn.onclick = (e) => {
            e.stopPropagation();
            openPreferencesModal();
        };

        actionsGroup.appendChild(prefsBtn);

        // Window controls (minimize, maximize, close)
        const winControls = document.createElement('div');
        winControls.className = 'nl-window-controls';

        const minBtn = document.createElement('button');
        minBtn.className = 'nl-btn nl-win-btn';
        minBtn.title = 'Minimize';
        minBtn.innerHTML = '<svg width="10" height="10" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5"><path d="M5 12h14"/></svg>';
        minBtn.onclick = async (e) => {
            e.stopPropagation();
            try {
                await safeInvoke('minimize_window', { windowId: currentWindowId });
            } catch {
                window.location.href = 'notion-lite-action://window:minimize';
            }
        };

        const maxBtn = document.createElement('button');
        maxBtn.className = 'nl-btn nl-win-btn';
        maxBtn.title = 'Maximize / Restore';
        maxBtn.innerHTML = '<svg width="10" height="10" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><rect x="3" y="3" width="18" height="18" rx="2"/></svg>';
        maxBtn.onclick = async (e) => {
            e.stopPropagation();
            try {
                await safeInvoke('maximize_window', { windowId: currentWindowId });
            } catch {
                window.location.href = 'notion-lite-action://window:maximize';
            }
        };

        const closeBtn = document.createElement('button');
        closeBtn.className = 'nl-btn nl-win-btn nl-win-close';
        closeBtn.title = 'Close';
        closeBtn.innerHTML = '<svg width="11" height="11" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5"><path d="M18 6L6 18M6 6l12 12"/></svg>';
        closeBtn.onclick = async (e) => {
            e.stopPropagation();
            try {
                await safeInvoke('close_window', { windowId: currentWindowId });
            } catch {
                window.location.href = 'notion-lite-action://window:close';
            }
        };

        winControls.appendChild(minBtn);
        winControls.appendChild(maxBtn);
        winControls.appendChild(closeBtn);
        actionsGroup.appendChild(winControls);

        header.addEventListener('dblclick', async (e) => {
            if (e.target === header || e.target === tabsStrip) {
                try {
                    await safeInvoke('maximize_window', { windowId: currentWindowId });
                } catch {
                    window.location.href = 'notion-action://window:maximize';
                }
            }
        });

        header.appendChild(navGroup);
        header.appendChild(tabsStrip);
        header.appendChild(actionsGroup);

        document.body.appendChild(header);

        // 4. Preferences Modal (Lazily created on demand to reduce per-tab DOM/memory overhead)
        let modalBackdrop = null;
        let selectedTheme = initialTheme;

        const updateThemeSelection = (theme) => {
            selectedTheme = theme;
            if (modalBackdrop) {
                modalBackdrop.querySelectorAll('.nl-theme-card').forEach(card => {
                    card.classList.toggle('selected', card.getAttribute('data-theme') === theme);
                });
            }
        };

        const closePreferencesModal = () => {
            if (modalBackdrop) modalBackdrop.style.display = 'none';
        };

        const ensurePreferencesModal = () => {
            if (modalBackdrop) return;
            modalBackdrop = document.createElement('div');
            modalBackdrop.id = 'notion-lite-prefs-modal';
            modalBackdrop.className = 'nl-modal-backdrop';
            modalBackdrop.style.display = 'none';

            modalBackdrop.innerHTML = `
            <div class="nl-modal-card">
                <div class="nl-modal-header">
                    <div class="nl-modal-title">
                        <span>⚙️</span>
                        <span>Notion Lite Preferences</span>
                    </div>
                    <button class="nl-modal-close" id="nl-prefs-close">×</button>
                </div>
                <div class="nl-modal-body">
                    <div>
                        <div class="nl-section-title">Theme & Appearance</div>
                        <div class="nl-theme-grid">
                            <div class="nl-theme-card" data-theme="dracula">🧛 Dracula</div>
                            <div class="nl-theme-card" data-theme="nord">❄️ Nord</div>
                            <div class="nl-theme-card" data-theme="default">☀️ Light</div>
                        </div>
                    </div>

                    <div>
                        <div class="nl-section-title">Window & Menus</div>
                        <div class="nl-field-row">
                            <div>
                                <div class="nl-field-label">Show Native Menu Bar</div>
                                <div class="nl-field-desc">Toggle OS menu bar (or press Ctrl+Shift+M)</div>
                            </div>
                            <label class="nl-switch">
                                <input type="checkbox" id="nl-cfg-menu-bar">
                                <span class="nl-slider"></span>
                            </label>
                        </div>
                        <div class="nl-field-row">
                            <div>
                                <div class="nl-field-label">Restore Tabs on Startup</div>
                                <div class="nl-field-desc">Automatically restore previously opened tabs</div>
                            </div>
                            <label class="nl-switch">
                                <input type="checkbox" id="nl-cfg-restore-tabs">
                                <span class="nl-slider"></span>
                            </label>
                        </div>
                        <div style="margin-top: 10px;">
                            <div class="nl-field-label">Startup URL</div>
                            <input type="text" id="nl-cfg-start-url" class="nl-input" placeholder="https://app.notion.com">
                        </div>
                    </div>

                    <div>
                        <div class="nl-section-title">Maintenance & Shortcuts</div>
                        <div class="nl-btn-row">
                            <button class="nl-action-btn nl-btn-sec" id="nl-btn-check-updates">Check for Updates</button>
                            <button class="nl-action-btn nl-btn-sec" id="nl-btn-open-config">Open Config Folder</button>
                        </div>
                        <div class="nl-status-text" id="nl-update-status"></div>
                    </div>
                </div>
                <div class="nl-modal-footer">
                    <span style="font-size: 11px; opacity: 0.6;" id="nl-version-label">Notion Lite</span>
                    <div class="nl-btn-row">
                        <button class="nl-action-btn nl-btn-sec" id="nl-btn-cancel">Cancel</button>
                        <button class="nl-action-btn nl-btn-pri" id="nl-btn-save">Save & Apply</button>
                    </div>
                </div>
            </div>
        `;

            document.body.appendChild(modalBackdrop);

            // Attach Modal Events
            modalBackdrop.addEventListener('click', (e) => {
                if (e.target === modalBackdrop) closePreferencesModal();
            });
            document.getElementById('nl-prefs-close').onclick = closePreferencesModal;
            document.getElementById('nl-btn-cancel').onclick = closePreferencesModal;

            modalBackdrop.querySelectorAll('.nl-theme-card').forEach(card => {
                card.onclick = () => {
                    const th = card.getAttribute('data-theme');
                    if (th) updateThemeSelection(th);
                };
            });

            document.getElementById('nl-btn-check-updates').onclick = async () => {
                const statusEl = document.getElementById('nl-update-status');
                statusEl.textContent = 'Checking for updates...';
                try {
                    const hasUpdate = await safeInvoke('check_for_updates_command');
                    statusEl.textContent = hasUpdate ? 'Update available! Downloading...' : 'App is up to date.';
                } catch (err) {
                    statusEl.textContent = 'Update check: ' + (err.message || err);
                }
            };

            document.getElementById('nl-btn-open-config').onclick = async () => {
                try {
                    await safeInvoke('open_config_directory');
                } catch {
                    window.location.href = 'notion-lite-action://config:open_dir';
                }
            };

            document.getElementById('nl-btn-save').onclick = async () => {
                const showMenuBar = document.getElementById('nl-cfg-menu-bar').checked;
                const restoreTabs = document.getElementById('nl-cfg-restore-tabs').checked;
                const startUrl = document.getElementById('nl-cfg-start-url').value;

                // Apply theme immediately to current window
                document.documentElement.setAttribute('data-nl-theme', selectedTheme);

                try {
                    await safeInvoke('update_app_config', {
                        payload: {
                            active_theme: selectedTheme,
                            show_menu_bar: showMenuBar,
                            restore_tabs: restoreTabs,
                            start_url: startUrl
                        }
                    });
                } catch (err) {
                    console.warn('[NotionLite] IPC update_app_config error:', err);
                }
                closePreferencesModal();
                renderTabs();
            };
        };

        const openPreferencesModal = async () => {
            ensurePreferencesModal();
            modalBackdrop.style.display = 'flex';
            const curTheme = document.documentElement.getAttribute('data-nl-theme') || initialTheme;
            updateThemeSelection(curTheme);

            try {
                const cfg = await safeInvoke('get_app_config');
                if (cfg) {
                    selectedTheme = cfg.active_theme || curTheme;
                    updateThemeSelection(selectedTheme);
                    const mb = document.getElementById('nl-cfg-menu-bar');
                    if (mb) mb.checked = !!cfg.show_menu_bar;
                    const rt = document.getElementById('nl-cfg-restore-tabs');
                    if (rt) rt.checked = !!cfg.restore_tabs;
                    const su = document.getElementById('nl-cfg-start-url');
                    if (su) su.value = cfg.start_url || '';
                    const vl = document.getElementById('nl-version-label');
                    if (vl) vl.textContent = 'Notion Lite v' + (cfg.version || '0.3.30');
                    const us = document.getElementById('nl-update-status');
                    if (us) us.textContent = '';
                }
            } catch (e) {
                console.warn('[NotionLite] Could not fetch live app config via IPC:', e);
            }
        };

        // Keyboard Shortcuts
        window.addEventListener('keydown', (e) => {
            if ((e.ctrlKey || e.metaKey) && e.key === ',') {
                e.preventDefault();
                openPreferencesModal();
            }
            if (e.key === 'Escape') {
                closePreferencesModal();
            }
            if ((e.ctrlKey || e.metaKey) && e.shiftKey && (e.key === 'M' || e.key === 'm')) {
                e.preventDefault();
                safeInvoke('toggle_menu_bar_command').catch(() => {});
            }
        });

        // Background Throttling & RAM Optimization Handlers
        let isBackgrounded = false;
        let originalRaf = window.requestAnimationFrame;

        const stubRaf = (callback) => {
            if (!isBackgrounded) {
                return originalRaf ? originalRaf(callback) : setTimeout(() => callback(performance.now()), 16);
            }
            return setTimeout(() => {
                if (typeof callback === 'function') {
                    callback(performance.now());
                }
            }, 1000);
        };

        const onBackground = () => {
            if (isBackgrounded) return;
            isBackgrounded = true;
            console.log('[NotionLite] Background mode active: throttling UI rendering & media');

            try {
                document.querySelectorAll('audio, video').forEach(el => {
                    if (!el.paused && typeof el.pause === 'function') {
                        el.pause();
                    }
                });
            } catch (e) {
                console.warn('[NotionLite] Pause media error:', e);
            }

            try {
                if (window.requestAnimationFrame !== stubRaf) {
                    originalRaf = window.requestAnimationFrame;
                    window.requestAnimationFrame = stubRaf;
                }
            } catch (e) {
                console.warn('[NotionLite] Override rAF error:', e);
            }

            try {
                if (typeof window.gc === 'function') {
                    window.gc();
                }
            } catch (e) {}
        };

        const onForeground = () => {
            if (!isBackgrounded) return;
            isBackgrounded = false;
            console.log('[NotionLite] Foreground mode restored: resuming full UI activity');

            try {
                if (originalRaf) {
                    window.requestAnimationFrame = originalRaf;
                }
            } catch (e) {
                console.warn('[NotionLite] Restore rAF error:', e);
            }

            renderTabs();
        };

        // Global helper object
        window.NotionLite = {
            tabId: currentTabId,
            windowId: currentWindowId,
            openPreferences: openPreferencesModal,
            closePreferences: closePreferencesModal,
            refreshTabs: renderTabs,
            onBackground: onBackground,
            onForeground: onForeground,
            invoke: safeInvoke
        };

        // Initial render and lifecycle listeners
        renderTabs();
        setTimeout(renderTabs, 500);
        setTimeout(renderTabs, 1500);
        window.addEventListener('focus', () => {
            onForeground();
            renderTabs();
        });
        document.addEventListener('visibilitychange', () => {
            if (document.hidden) {
                onBackground();
            } else {
                onForeground();
                renderTabs();
            }
        });
    }

    runInit();
})();
