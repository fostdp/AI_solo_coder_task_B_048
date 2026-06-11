const API_BASE = window.location.origin.includes('localhost') && !window.location.origin.includes('8080')
    ? 'http://localhost:8080'
    : '';

const App = (() => {
    const state = {
        locations: [],
        heatmapRange: 168,
        currentFeature: null,
        currentProbe: null,
    };

    async function fetchJSON(path) {
        try {
            const res = await fetch(API_BASE + path);
            const json = await res.json();
            return json.success ? json.data : null;
        } catch (e) {
            console.error('Request failed:', path, e);
            return null;
        }
    }

    async function refreshStats() {
        const stats = await fetchJSON('/api/stats');
        if (!stats) return;
        const totalEl = document.getElementById('stat-total');
        const riskEl = document.getElementById('stat-risk');
        const avgEl = document.getElementById('stat-avg');
        if (totalEl) totalEl.textContent = (stats.total_soil_sensors + stats.total_corrosion_probes);
        if (riskEl) riskEl.textContent = stats.high_risk_probes;
        if (avgEl) avgEl.textContent = stats.avg_corrosion_rate?.toFixed(4) + ' mm/y';
    }

    async function loadLocations() {
        state.locations = await fetchJSON('/api/locations') || [];
        renderDeviceList('all');
        MapView.renderMarkers(state.locations, handleProbeClick);
    }

    function renderDeviceList(tab) {
        const listEl = document.getElementById('device-list');
        if (!listEl) return;
        const items = state.locations.filter(loc => {
            if (tab === 'all') return true;
            if (tab === 'soil') return loc.device_type === 'soil_sensor';
            if (tab === 'corrosion') return loc.device_type === 'corrosion_probe';
            return true;
        });
        if (items.length === 0) {
            listEl.innerHTML = '<div class="loading">无设备</div>';
            return;
        }
        let html = '';
        for (const loc of items) {
            const dotClass = loc.device_type === 'soil_sensor' ? 'soil'
                : (loc.material_type === 'iron' ? 'iron' : 'copper');
            html += `<div class="device-item" data-id="${loc.device_id}">
                <div class="device-dot ${dotClass}"></div>
                <div class="device-info">
                    <div class="device-name">${loc.device_name}</div>
                    <div class="device-zone">${loc.zone}</div>
                </div>
            </div>`;
        }
        listEl.innerHTML = html;
        listEl.querySelectorAll('.device-item').forEach(el => {
            el.addEventListener('click', () => {
                const id = el.getAttribute('data-id');
                const loc = state.locations.find(l => l.device_id === id);
                if (loc) handleProbeClick(loc);
            });
        });
    }

    async function refreshHeatmap() {
        const data = await fetchJSON(`/api/corrosion/heatmap?hours=${state.heatmapRange}`);
        if (!data) return;
        MapView.setHeatmapData(data);
    }

    async function handleProbeClick(probe) {
        state.currentProbe = probe;
        openModal(probe.device_name);
        const modalBody = document.getElementById('modal-body');
        if (!modalBody) return;

        if (probe.device_type === 'soil_sensor') {
            modalBody.innerHTML = `
                <div class="detail-section">
                    <h3>设备信息</h3>
                    <div class="info-grid">
                        <div class="info-box"><div class="info-box-label">设备ID</div><div class="info-box-value">${probe.device_id}</div></div>
                        <div class="info-box"><div class="info-box-label">区域</div><div class="info-box-value">${probe.zone}</div></div>
                        <div class="info-box"><div class="info-box-label">类型</div><div class="info-box-value">土壤传感器</div></div>
                        <div class="info-box"><div class="info-box-label">经纬度</div><div class="info-box-value">${probe.lat.toFixed(4)}, ${probe.lng.toFixed(4)}</div></div>
                    </div>
                </div>
                <div class="detail-section">
                    <h3>说明</h3>
                    <p style="color:#94a3b8;font-size:13px;line-height:1.7">该设备监测土壤温湿度、pH值、氯离子浓度等微环境参数，数据通过LoRa网络上传至监测中心。</p>
                </div>`;
            return;
        }

        CorrosionPanel.clear();
        VulnerabilityPanel.clear && VulnerabilityPanel.clear();
        modalBody.innerHTML = `
            <div class="sub-tabs">
                <button class="sub-tab-btn active" data-sub="corrosion">腐蚀趋势</button>
                <button class="sub-tab-btn" data-sub="vulnerability">脆弱性指数</button>
            </div>
            <div id="sub-panel-corrosion" class="sub-panel active"></div>
            <div id="sub-panel-vulnerability" class="sub-panel"></div>
        `;

        const corrosionEl = document.getElementById('sub-panel-corrosion');
        CorrosionPanel.init(corrosionEl);
        const vulnEl = document.getElementById('sub-panel-vulnerability');
        VulnerabilityPanel.init(vulnEl);

        bindSubTabs();

        const [trend, prediction, stability] = await Promise.all([
            fetchJSON(`/api/corrosion/trend/${probe.device_id}?hours=${state.heatmapRange}`),
            fetchJSON(`/api/corrosion/prediction/${probe.device_id}`),
            fetchJSON(`/api/corrosion/stability/${probe.device_id}`),
        ]);
        CorrosionPanel.render(probe, {
            trend: trend || [],
            prediction: prediction || null,
            stability: stability || null,
        });
    }

    function bindSubTabs() {
        document.querySelectorAll('.sub-tab-btn').forEach(btn => {
            btn.addEventListener('click', () => {
                const tab = btn.getAttribute('data-sub');
                document.querySelectorAll('.sub-tab-btn').forEach(b => b.classList.remove('active'));
                document.querySelectorAll('.sub-panel').forEach(p => p.classList.remove('active'));
                btn.classList.add('active');
                const panel = document.getElementById('sub-panel-' + tab);
                if (panel) panel.classList.add('active');
                if (tab === 'vulnerability' && state.currentProbe) {
                    VulnerabilityPanel.load(state.currentProbe.device_id);
                }
            });
        });
    }

    function bindDeviceTabs() {
        document.querySelectorAll('.tabs .tab-btn').forEach(btn => {
            btn.addEventListener('click', () => {
                document.querySelectorAll('.tabs .tab-btn').forEach(b => b.classList.remove('active'));
                btn.classList.add('active');
                renderDeviceList(btn.getAttribute('data-tab'));
            });
        });
    }

    function bindFeatureTabs() {
        document.querySelectorAll('.feature-btn').forEach(btn => {
            btn.addEventListener('click', () => {
                const feature = btn.getAttribute('data-feature');
                openFeaturePanel(feature);
            });
        });
    }

    function openFeaturePanel(feature) {
        state.currentFeature = feature;
        const titles = {
            vulnerability: '文物脆弱性指数分析',
            protection: '保护材料渗透深度模拟',
            microbiome: '土壤微生物活性与腐蚀关联',
            groundwater: '地下水动态预警与氯离子扩散',
        };
        openModal(titles[feature] || '分析面板', true);
        const modalBody = document.getElementById('modal-body');
        if (!modalBody) return;
        modalBody.innerHTML = '<div id="feature-panel-content"></div>';
        const content = document.getElementById('feature-panel-content');

        if (feature === 'vulnerability') {
            VulnerabilityPanel.init(content);
            VulnerabilityPanel.loadAll();
        } else if (feature === 'protection') {
            ProtectionPanel.init(content);
        } else if (feature === 'microbiome') {
            MicrobiomePanel.init(content);
            MicrobiomePanel.load();
        } else if (feature === 'groundwater') {
            GroundwaterPanel.init(content);
        }
    }

    function openModal(title, wide) {
        const modal = document.getElementById('detail-modal');
        const titleEl = document.getElementById('modal-title');
        const content = document.querySelector('.modal-content');
        if (titleEl) titleEl.textContent = title;
        if (wide && content) content.style.maxWidth = '960px';
        else if (content) content.style.maxWidth = '720px';
        if (modal) modal.classList.add('active');
    }

    function closeModal() {
        const modal = document.getElementById('detail-modal');
        if (modal) modal.classList.remove('active');
        state.currentFeature = null;
        state.currentProbe = null;
    }

    function bindControls() {
        document.getElementById('close-modal')?.addEventListener('click', closeModal);
        document.getElementById('detail-modal')?.addEventListener('click', (e) => {
            if (e.target.id === 'detail-modal') closeModal();
        });
        document.getElementById('btn-refresh')?.addEventListener('click', refreshAll);
        document.getElementById('toggle-heatmap')?.addEventListener('change', (e) => {
            MapView.toggleHeatmap(e.target.checked);
        });
        document.getElementById('toggle-soil')?.addEventListener('change', (e) => {
            MapView.toggleLayer('soil', e.target.checked);
        });
        document.getElementById('toggle-corrosion')?.addEventListener('change', (e) => {
            MapView.toggleLayer('corrosion', e.target.checked);
        });
        document.getElementById('heatmap-range')?.addEventListener('change', (e) => {
            state.heatmapRange = parseInt(e.target.value) || 168;
            refreshHeatmap();
        });
    }

    async function refreshAll() {
        await Promise.all([refreshStats(), refreshHeatmap()]);
    }

    async function init() {
        MapView.init(handleProbeClick);
        CorrosionPanel.init(null);
        bindControls();
        bindDeviceTabs();
        bindFeatureTabs();
        await loadLocations();
        await refreshAll();
        setInterval(refreshAll, 60 * 1000);
    }

    return {
        init,
        apiBase: API_BASE,
    };
})();

document.addEventListener('DOMContentLoaded', App.init);
