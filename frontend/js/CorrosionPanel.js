const CorrosionPanel = (() => {
    let trendCanvas = null;
    let trendCtx = null;
    let currentProbe = null;

    function init() {
        trendCanvas = document.getElementById('trend-chart');
        if (trendCanvas) {
            trendCtx = trendCanvas.getContext('2d');
            resizeTrendCanvas();
            window.addEventListener('resize', () => {
                resizeTrendCanvas();
                if (currentProbe) renderTrend(currentProbe.trendData || []);
            });
        }
    }

    function resizeTrendCanvas() {
        if (!trendCanvas) return;
        const parent = trendCanvas.parentElement;
        const w = parent.clientWidth;
        const h = Math.max(200, Math.min(320, parent.clientHeight || 240));
        const dpr = window.devicePixelRatio || 1;
        trendCanvas.width = w * dpr;
        trendCanvas.height = h * dpr;
        trendCanvas.style.width = w + 'px';
        trendCanvas.style.height = h + 'px';
        trendCtx.setTransform(dpr, 0, 0, dpr, 0, 0);
    }

    function showLoading() {
        const panel = document.getElementById('probe-detail');
        if (!panel) return;
        panel.style.display = 'block';
        panel.innerHTML = `
            <div style="padding:40px;text-align:center;color:#888;">
                <div class="spinner" style="margin:0 auto 16px;"></div>
                正在加载探针数据...
            </div>`;
    }

    function render(probe, detail) {
        currentProbe = { ...probe, ...detail };
        const panel = document.getElementById('probe-detail');
        if (!panel) return;
        panel.style.display = 'block';

        const { trend, prediction, stability } = detail;
        currentProbe.trendData = trend;

        const trendHtml = `
            <div style="position:relative;">
                <canvas id="trend-chart" style="width:100%;height:220px;"></canvas>
            </div>
        `;

        const predHtml = prediction ? `
            <div style="margin:18px 0 8px;font-weight:600;color:#ffcc80;">📈 腐蚀速率预测（基于NN+环境因子）</div>
            <div style="display:grid;grid-template-columns:repeat(3,1fr);gap:10px;">
                <div class="stat-card"><div class="stat-label">7天</div><div class="stat-value" style="color:${getRateColor(prediction.predicted_rate_7d)}">${prediction.predicted_rate_7d.toFixed(4)}</div></div>
                <div class="stat-card"><div class="stat-label">30天</div><div class="stat-value" style="color:${getRateColor(prediction.predicted_rate_30d)}">${prediction.predicted_rate_30d.toFixed(4)}</div></div>
                <div class="stat-card"><div class="stat-label">90天</div><div class="stat-value" style="color:${getRateColor(prediction.predicted_rate_90d)}">${prediction.predicted_rate_90d.toFixed(4)}</div></div>
            </div>
            <div style="margin-top:10px;display:flex;align-items:center;gap:10px;">
                <span style="color:#888;font-size:12px;">风险等级:</span>
                <span class="risk-badge risk-${getRiskKey(prediction.risk_level)}">${prediction.risk_level || '中'}</span>
                <span style="color:#666;font-size:12px;">置信度 ${(prediction.confidence || 0.85).toFixed(1)}%</span>
            </div>
        ` : '';

        const stabHtml = stability ? `
            <div style="margin:18px 0 8px;font-weight:600;color:#81d4fa;">🛡️ ${stability.material_type === 'copper' ? '铜器' : '铁器'}稳定性评估</div>
            <div style="display:grid;grid-template-columns:repeat(2,1fr);gap:10px;">
                <div class="stat-card"><div class="stat-label">稳定指数</div><div class="stat-value" style="color:${(stability.stability_index || 0) > 0.5 ? '#4caf50' : '#f44336'}">${(stability.stability_index || 0).toFixed(3)}</div></div>
                <div class="stat-card"><div class="stat-label">稳定等级</div><div class="stat-value" style="color:#ffb74d;">${stability.stability_level || '-'}</div></div>
                <div class="stat-card"><div class="stat-label">环境评分</div><div class="stat-value">${(stability.env_score || 0).toFixed(2)}</div></div>
                <div class="stat-card"><div class="stat-label">预估剩余年限</div><div class="stat-value">${(stability.remaining_lifetime_years || 0).toFixed(1)}年</div></div>
            </div>
            ${stability.recommendations && stability.recommendations.length ? `
                <div style="margin-top:12px;">
                    <div style="font-size:13px;color:#90caf9;margin-bottom:6px;">💡 保护建议</div>
                    <ul style="margin:0;padding-left:20px;color:#bbb;font-size:13px;">
                        ${stability.recommendations.map(r => `<li style="margin:4px 0;">${r}</li>`).join('')}
                    </ul>
                </div>
            ` : ''}
        ` : '';

        panel.innerHTML = `
            <div style="display:flex;justify-content:space-between;align-items:center;margin-bottom:14px;padding-bottom:12px;border-bottom:1px solid #2a2a3a;">
                <div>
                    <div style="font-size:17px;font-weight:700;">${probe.device_name}</div>
                    <div style="color:#888;font-size:12px;">${probe.device_id} · ${probe.zone} · ${materialLabel(probe.material_type)}</div>
                </div>
                <button class="close-btn" onclick="document.getElementById('probe-detail').style.display='none'">×</button>
            </div>
            ${trendHtml}
            ${predHtml}
            ${stabHtml}
        `;

        trendCanvas = document.getElementById('trend-chart');
        if (trendCanvas) {
            trendCtx = trendCanvas.getContext('2d');
            resizeTrendCanvas();
            renderTrend(trend || []);
        }
    }

    function renderTrend(data) {
        if (!trendCtx) return;
        const w = parseFloat(trendCanvas.style.width);
        const h = parseFloat(trendCanvas.style.height);
        if (w < 20 || h < 20) return;

        trendCtx.clearRect(0, 0, w, h);

        const padL = 50, padR = 16, padT = 18, padB = 30;
        const chartW = w - padL - padR;
        const chartH = h - padT - padB;

        const threshold = 0.5;
        const maxRate = data.length ? Math.max(threshold * 1.1, ...data.map(d => d.corrosion_rate || 0)) : threshold * 1.2;
        const minTime = data.length ? Math.min(...data.map(d => d.time || 0)) : Date.now() - 7 * 86400000;
        const maxTime = data.length ? Math.max(...data.map(d => d.time || 0)) : Date.now();
        const timeSpan = Math.max(1, maxTime - minTime);

        trendCtx.strokeStyle = '#1e1e2e';
        trendCtx.lineWidth = 1;
        for (let i = 0; i <= 4; i++) {
            const y = padT + (chartH / 4) * i;
            trendCtx.beginPath();
            trendCtx.moveTo(padL, y);
            trendCtx.lineTo(w - padR, y);
            trendCtx.stroke();
        }

        const threshY = padT + chartH * (1 - threshold / maxRate);
        trendCtx.strokeStyle = '#f44336';
        trendCtx.setLineDash([6, 4]);
        trendCtx.beginPath();
        trendCtx.moveTo(padL, threshY);
        trendCtx.lineTo(w - padR, threshY);
        trendCtx.stroke();
        trendCtx.setLineDash([]);
        trendCtx.fillStyle = '#f44336';
        trendCtx.font = '10px sans-serif';
        trendCtx.textAlign = 'left';
        trendCtx.fillText(`告警阈值 ${threshold.toFixed(1)} mm/y`, w - padR - 120, threshY - 4);

        trendCtx.fillStyle = '#888';
        trendCtx.font = '11px sans-serif';
        trendCtx.textAlign = 'right';
        for (let i = 0; i <= 4; i++) {
            const y = padT + (chartH / 4) * (4 - i);
            const v = (maxRate / 4) * i;
            trendCtx.fillText(v.toFixed(2), padL - 6, y + 4);
        }

        trendCtx.textAlign = 'center';
        for (let i = 0; i <= 3; i++) {
            const t = minTime + (timeSpan / 3) * i;
            const x = padL + (chartW / 3) * i;
            const d = new Date(t);
            trendCtx.fillText(`${d.getMonth()+1}/${d.getDate()}`, x, h - padB + 16);
        }

        if (data.length === 0) {
            trendCtx.fillStyle = '#555';
            trendCtx.textAlign = 'center';
            trendCtx.font = '13px sans-serif';
            trendCtx.fillText('暂无历史数据', padL + chartW / 2, padT + chartH / 2);
            return;
        }

        const grad = trendCtx.createLinearGradient(0, padT, 0, padT + chartH);
        grad.addColorStop(0, 'rgba(255, 152, 0, 0.35)');
        grad.addColorStop(1, 'rgba(255, 152, 0, 0)');
        trendCtx.fillStyle = grad;
        trendCtx.beginPath();
        data.forEach((p, i) => {
            const x = padL + chartW * ((p.time || 0) - minTime) / timeSpan;
            const y = padT + chartH * (1 - Math.min(maxRate, p.corrosion_rate || 0) / maxRate);
            if (i === 0) trendCtx.moveTo(x, padT + chartH);
            trendCtx.lineTo(x, y);
        });
        const lastX = padL + chartW;
        trendCtx.lineTo(lastX, padT + chartH);
        trendCtx.closePath();
        trendCtx.fill();

        trendCtx.strokeStyle = '#ff9800';
        trendCtx.lineWidth = 2;
        trendCtx.beginPath();
        data.forEach((p, i) => {
            const x = padL + chartW * ((p.time || 0) - minTime) / timeSpan;
            const y = padT + chartH * (1 - Math.min(maxRate, p.corrosion_rate || 0) / maxRate);
            if (i === 0) trendCtx.moveTo(x, y); else trendCtx.lineTo(x, y);
        });
        trendCtx.stroke();

        data.forEach(p => {
            const x = padL + chartW * ((p.time || 0) - minTime) / timeSpan;
            const y = padT + chartH * (1 - Math.min(maxRate, p.corrosion_rate || 0) / maxRate);
            trendCtx.fillStyle = (p.corrosion_rate || 0) > threshold ? '#f44336' : '#ffcc80';
            trendCtx.beginPath();
            trendCtx.arc(x, y, 2.5, 0, Math.PI * 2);
            trendCtx.fill();
        });
    }

    function getRateColor(r) {
        if (!r || r < 0.1) return '#4caf50';
        if (r < 0.3) return '#ffb74d';
        if (r < 0.5) return '#ff9800';
        return '#f44336';
    }

    function getRiskKey(level) {
        if (!level) return 'medium';
        const map = { '低': 'low', '中': 'medium', '较高': 'high', '高': 'critical' };
        return map[level] || 'medium';
    }

    function materialLabel(t) {
        return t === 'copper' ? '铜质' : t === 'iron' ? '铁质' : '其他';
    }

    return { init, showLoading, render };
})();
