const API_BASE = window.location.origin.includes('localhost') && !window.location.origin.includes('8080')
    ? 'http://localhost:8080'
    : '';

const App = (() => {
    const state = {
        locations: [],
        heatmapRange: 168,
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
        const ids = ['soil-count', 'probe-count', 'zone-count', 'high-risk-count',
            'avg-rate', 'avg-temp', 'avg-hum', 'avg-ph', 'avg-cl'];
        const vals = [
            stats.total_soil_sensors,
            stats.total_corrosion_probes,
            stats.total_zones,
            stats.high_risk_probes,
            stats.avg_corrosion_rate?.toFixed(4),
            stats.avg_temperature?.toFixed(1) + ' °C',
            stats.avg_humidity?.toFixed(1) + '%',
            stats.avg_ph?.toFixed(2),
            stats.avg_chloride?.toFixed(1) + ' ppm',
        ];
        ids.forEach((id, i) => { const el = document.getElementById(id); if (el) el.textContent = vals[i]; });
    }

    async function loadLocations() {
        state.locations = await fetchJSON('/api/locations') || [];
        MapView.renderMarkers(state.locations);
    }

    async function refreshHeatmap() {
        const data = await fetchJSON(`/api/corrosion/heatmap?hours=${state.heatmapRange}`);
        if (!data) return;
        MapView.setHeatmapData(data);
        const el = document.getElementById('heatmap-range-label');
        if (el) {
            const h = state.heatmapRange;
            el.textContent = h < 48 ? `${h}小时` : `${(h/24).toFixed(0)}天`;
        }
    }

    async function handleProbeClick(probe) {
        CorrosionPanel.showLoading();
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

    function setupHeatmapButtons() {
        const map = { 'heat-24h': 24, 'heat-7d': 168, 'heat-30d': 720 };
        Object.entries(map).forEach(([id, hours]) => {
            const btn = document.getElementById(id);
            if (!btn) return;
            btn.addEventListener('click', () => {
                Object.keys(map).forEach(i => {
                    document.getElementById(i)?.classList.remove('active');
                });
                btn.classList.add('active');
                state.heatmapRange = hours;
                refreshHeatmap();
            });
        });
    }

    async function refreshAll() {
        await Promise.all([refreshStats(), refreshHeatmap()]);
    }

    async function init() {
        MapView.init(handleProbeClick);
        CorrosionPanel.init();
        setupHeatmapButtons();
        await loadLocations();
        await refreshAll();

        document.getElementById('btn-refresh')?.addEventListener('click', refreshAll);
        setInterval(refreshAll, 60 * 1000);
    }

    return { init };
})();

document.addEventListener('DOMContentLoaded', App.init);
