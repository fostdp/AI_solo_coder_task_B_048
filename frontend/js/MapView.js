const MapView = (() => {
    const CENTER_LAT = 34.2658;
    const CENTER_LNG = 108.9542;
    const SITE_RADIUS = 0.0012;

    let map = null;
    let heatmapCanvas = null;
    let heatmapCtx = null;
    let markersLayer = null;
    let soilLayer = null;
    let corrosionLayer = null;
    let siteBounds = null;
    let heatmapData = [];
    let allLocations = [];
    let onProbeClick = null;
    let heatmapVisible = true;

    function init(onClickCallback) {
        onProbeClick = onClickCallback;

        map = L.map('map', {
            center: [CENTER_LAT, CENTER_LNG],
            zoom: 18,
            zoomControl: true,
            minZoom: 16,
            maxZoom: 21,
            preferCanvas: true,
        });

        L.tileLayer('https://{s}.tile.openstreetmap.org/{z}/{x}/{y}.png', {
            attribution: '&copy; OpenStreetMap contributors',
            maxNativeZoom: 19,
            maxZoom: 21,
        }).addTo(map);

        markersLayer = L.layerGroup().addTo(map);
        soilLayer = L.layerGroup().addTo(map);
        corrosionLayer = L.layerGroup().addTo(map);

        drawSiteBoundary();
        createHeatmapCanvas();

        map.on('moveend zoomend resize', () => {
            updateHeatmapCanvas();
            renderHeatmap();
        });
    }

    function drawSiteBoundary() {
        const points = [];
        const sides = 8;
        for (let i = 0; i < sides; i++) {
            const angle = (i / sides) * Math.PI * 2;
            points.push([
                CENTER_LAT + SITE_RADIUS * Math.cos(angle),
                CENTER_LNG + SITE_RADIUS * Math.sin(angle) * 0.8,
            ]);
        }
        L.polygon(points, {
            color: '#ff9800',
            weight: 2,
            fillColor: '#ff9800',
            fillOpacity: 0.05,
            dashArray: '8,6',
        }).addTo(map).bindTooltip('宋代战地医院遗址（2000㎡）', {
            permanent: false,
            direction: 'top',
        });

        siteBounds = L.latLngBounds(points);
    }

    function createHeatmapCanvas() {
        const pane = map.getPane('overlayPane');
        heatmapCanvas = document.createElement('canvas');
        heatmapCanvas.id = 'heatmap-canvas';
        heatmapCanvas.style.position = 'absolute';
        heatmapCanvas.style.pointerEvents = 'none';
        heatmapCanvas.style.zIndex = '450';
        pane.appendChild(heatmapCanvas);
        heatmapCtx = heatmapCanvas.getContext('2d');
        updateHeatmapCanvas();
    }

    function updateHeatmapCanvas() {
        if (!map) return;
        const topLeft = map.latLngToContainerPoint(siteBounds.getNorthWest());
        const bottomRight = map.latLngToContainerPoint(siteBounds.getSouthEast());
        const mapSize = map.getSize();

        heatmapCanvas.style.left = topLeft.x + 'px';
        heatmapCanvas.style.top = topLeft.y + 'px';
        const w = Math.max(1, bottomRight.x - topLeft.x);
        const h = Math.max(1, bottomRight.y - topLeft.y);

        const dpr = window.devicePixelRatio || 1;
        heatmapCanvas.width = w * dpr;
        heatmapCanvas.height = h * dpr;
        heatmapCanvas.style.width = w + 'px';
        heatmapCanvas.style.height = h + 'px';
        heatmapCtx.setTransform(dpr, 0, 0, dpr, 0, 0);
    }

    function renderMarkers(locations, onClickCallback) {
        if (onClickCallback) onProbeClick = onClickCallback;
        allLocations = locations;
        soilLayer.clearLayers();
        corrosionLayer.clearLayers();
        markersLayer.clearLayers();

        locations.forEach(loc => {
            const isSoil = loc.device_type === 'soil_sensor';
            const color = isSoil ? '#4caf50' : (loc.material_type === 'copper' ? '#ff9800' : '#f44336');
            const icon = L.divIcon({
                className: 'probe-icon',
                html: `<div deviceId="${loc.device_id}" style="
                    width:${isSoil ? 10 : 14}px;
                    height:${isSoil ? 10 : 14}px;
                    background:${color};
                    border:2px solid #fff;
                    border-radius:50%;
                    box-shadow:0 0 6px ${color}88;
                "></div>`,
                iconSize: [isSoil ? 14 : 18, isSoil ? 14 : 18],
                iconAnchor: [isSoil ? 7 : 9, isSoil ? 7 : 9],
            });

            const marker = L.marker([loc.lat, loc.lng], { icon });
            marker.bindTooltip(`${loc.device_name}<br><small>${loc.zone}</small>`, {
                offset: [0, -8], direction: 'top', opacity: 0.95,
            });

            if (!isSoil) {
                marker.on('click', () => {
                    highlightProbe(loc.device_id);
                    if (onProbeClick) onProbeClick(loc);
                });
                marker.addTo(corrosionLayer);
            } else {
                marker.on('click', () => {
                    if (onProbeClick) onProbeClick(loc);
                });
                marker.addTo(soilLayer);
            }
        });
    }

    function toggleHeatmap(visible) {
        heatmapVisible = visible;
        if (heatmapCanvas) {
            heatmapCanvas.style.display = visible ? 'block' : 'none';
            if (visible) renderHeatmap();
        }
    }

    function toggleLayer(layerType, visible) {
        if (!map) return;
        if (layerType === 'soil') {
            if (visible) map.addLayer(soilLayer); else map.removeLayer(soilLayer);
        } else if (layerType === 'corrosion') {
            if (visible) map.addLayer(corrosionLayer); else map.removeLayer(corrosionLayer);
        }
    }

    function highlightProbe(deviceId) {
        markersLayer.eachLayer(m => {
            const icon = m.getIcon && m.getIcon();
            if (!icon || !icon.options || !icon.options.html) return;
            const isActive = icon.options.html.includes('deviceId="' + deviceId + '"');
            const oldHtml = icon.options.html;
            const base = isActive ? oldHtml + '' : oldHtml;
            if (oldHtml.includes('deviceId')) {
                const dId = oldHtml.match(/deviceId="([^"]+)"/)?.[1];
                if (dId === deviceId) {
                    m.setIcon(L.divIcon({
                        ...icon.options,
                        className: 'probe-icon probe-icon-active',
                        html: oldHtml.replace('box-shadow:0 0 6px', 'box-shadow:0 0 14px')
                            .replace('border:2px', 'border:3px'),
                    }));
                } else {
                    m.setIcon(L.divIcon({
                        ...icon.options,
                        className: 'probe-icon',
                        html: oldHtml.replace('box-shadow:0 0 14px', 'box-shadow:0 0 6px')
                            .replace('border:3px', 'border:2px'),
                    }));
                }
            }
        });
    }

    function setHeatmapData(data) {
        heatmapData = data;
        renderHeatmap();
    }

    function getHeatColor(intensity) {
        const i = Math.max(0, Math.min(1, intensity));
        const r = Math.round(255 * Math.min(1, i * 2));
        const g = Math.round(200 * Math.max(0, 1 - i * 1.5));
        const b = Math.round(100 * Math.max(0, 0.8 - i));
        return `rgba(${r},${g},${b},0.7)`;
    }

    function renderHeatmap() {
        if (!heatmapCtx || !map || heatmapData.length === 0) return;
        const w = parseFloat(heatmapCanvas.style.width);
        const h = parseFloat(heatmapCanvas.style.height);
        if (w < 10 || h < 10) return;
        heatmapCtx.clearRect(0, 0, w, h);

        const nw = siteBounds.getNorthWest();
        const nwPx = map.latLngToContainerPoint(nw);

        for (const point of heatmapData) {
            const ll = L.latLng(point.lat, point.lng);
            const px = map.latLngToContainerPoint(ll);
            const x = px.x - nwPx.x;
            const y = px.y - nwPx.y;

            if (x < -60 || x > w + 60 || y < -60 || y > h + 60) continue;
            const intensity = point.intensity || 0;
            const radius = 30 + intensity * 50;
            const color = getHeatColor(intensity);

            const grad = heatmapCtx.createRadialGradient(x, y, 0, x, y, radius);
            const col = getHeatColor(intensity);
            grad.addColorStop(0, col);
            grad.addColorStop(0.5, color.replace(/[\d.]+\)/, (intensity * 0.4).toFixed(2) + ')'));
            grad.addColorStop(1, 'rgba(0,0,0,0)');
            heatmapCtx.fillStyle = grad;
            heatmapCtx.beginPath();
            heatmapCtx.arc(x, y, radius, 0, Math.PI * 2);
            heatmapCtx.fill();
        }
    }

    function focusDevice(deviceId) {
        const loc = allLocations.find(l => l.device_id === deviceId);
        if (loc && map) {
            map.flyTo([loc.lat, loc.lng], 19, { duration: 0.6 });
            highlightProbe(deviceId);
        }
    }

    return {
        init,
        renderMarkers,
        setHeatmapData,
        focusDevice,
        highlightProbe,
        toggleHeatmap,
        toggleLayer,
    };
})();
