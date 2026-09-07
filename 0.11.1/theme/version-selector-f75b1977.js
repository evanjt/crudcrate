// Version selector for mdBook
// Fetches /versions.json and injects a dropdown into the sidebar

(function() {
    'use strict';

    function createVersionSelector(versions) {
        const currentPath = window.location.pathname;

        // Determine current version from URL
        let currentVersion = 'latest';
        for (const v of versions) {
            if (currentPath.startsWith(v.url)) {
                currentVersion = v.version;
                break;
            }
        }

        // Create dropdown container
        const container = document.createElement('div');
        container.className = 'version-selector';

        // Create label
        const label = document.createElement('label');
        label.htmlFor = 'version-select';
        label.textContent = 'Version';

        // Create select element
        const select = document.createElement('select');
        select.id = 'version-select';
        select.setAttribute('aria-label', 'Select documentation version');

        for (const v of versions) {
            const option = document.createElement('option');
            option.value = v.url;
            option.textContent = v.version + (v.latest ? ' (latest)' : '');
            if (v.version === currentVersion) {
                option.selected = true;
            }
            select.appendChild(option);
        }

        // Handle version change
        select.addEventListener('change', function() {
            const newUrl = this.value;
            // Try to preserve the current page path
            const pagePath = currentPath.replace(/^\/[^/]+\//, '/');
            window.location.href = newUrl + pagePath.substring(1);
        });

        container.appendChild(label);
        container.appendChild(select);
        return container;
    }

    function init() {
        // Fetch versions.json from root
        fetch('/versions.json')
            .then(response => {
                if (!response.ok) throw new Error('versions.json not found');
                return response.json();
            })
            .then(versions => {
                if (!versions || versions.length === 0) return;

                const selector = createVersionSelector(versions);

                // Insert at top of sidebar, before the scrollbox
                const sidebar = document.querySelector('.sidebar');
                const scrollbox = document.querySelector('.sidebar-scrollbox');
                if (sidebar && scrollbox) {
                    sidebar.insertBefore(selector, scrollbox);
                }
            })
            .catch(err => {
                console.log('Version selector: ' + err.message);
            });
    }

    // Run when DOM is ready
    if (document.readyState === 'loading') {
        document.addEventListener('DOMContentLoaded', init);
    } else {
        init();
    }
})();
