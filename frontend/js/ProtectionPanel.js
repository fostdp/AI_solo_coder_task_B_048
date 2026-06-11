var ProtectionPanel = (function () {
    var container = null;
    var params = {
        material: 'Silicone',
        temperature: 20,
        humidity: 50,
        porosity: 0.15,
        concentration: 1.0,
        hours: 24
    };

    function init(el) {
        container = el;
        renderControls();
    }

    function renderControls() {
        if (!container) return;
        var html = '';
        html += '<div class="panel-header"><h3>保护材料渗透深度模拟</h3></div>';
        html += '<div class="sim-controls">';
        html += '<div class="form-row">';
        html += '<label>保护材料：</label>';
        html += '<select id="mat-select">';
        html += '<option value="Silicone">有机硅</option>';
        html += '<option value="Fluoropolymer">氟聚合物</option>';
        html += '<option value="Acrylate">丙烯酸酯(Paraloid B72)</option>';
        html += '<option value="Epoxy">环氧树脂</option>';
        html += '<option value="Paraffin">石蜡</option>';
        html += '<option value="NanoSiO2">纳米SiO2</option>';
        html += '</select>';
        html += '</div>';

        html += '<div class="form-row"><label>温度(℃)：</label><input type="range" id="temp-slider" min="5" max="40" value="20"><span id="temp-val">20</span></div>';
        html += '<div class="form-row"><label>湿度(%)：</label><input type="range" id="hum-slider" min="20" max="90" value="50"><span id="hum-val">50</span></div>';
        html += '<div class="form-row"><label>孔隙率：</label><input type="range" id="por-slider" min="0.05" max="0.5" step="0.01" value="0.15"><span id="por-val">0.15</span></div>';
        html += '<div class="form-row"><label>处理时间(h)：</label><input type="range" id="time-slider" min="1" max="720" step="1" value="24"><span id="time-val">24</span></div>';

        html += '<button class="btn-primary" id="run-sim-btn">开始模拟</button>';
        html += '</div>';
        html += '<div id="sim-result" class="sim-result"></div>';
        container.innerHTML = html;

        bindEvents();
    }

    function bindEvents() {
        document.getElementById('temp-slider').addEventListener('input', function (e) {
            document.getElementById('temp-val').textContent = e.target.value;
            params.temperature = parseFloat(e.target.value);
        });
        document.getElementById('hum-slider').addEventListener('input', function (e) {
            document.getElementById('hum-val').textContent = e.target.value;
            params.humidity = parseFloat(e.target.value);
        });
        document.getElementById('por-slider').addEventListener('input', function (e) {
            document.getElementById('por-val').textContent = e.target.value;
            params.porosity = parseFloat(e.target.value);
        });
        document.getElementById('time-slider').addEventListener('input', function (e) {
            document.getElementById('time-val').textContent = e.target.value;
            params.hours = parseFloat(e.target.value);
        });
        document.getElementById('mat-select').addEventListener('change', function (e) {
            params.material = e.target.value;
        });
        document.getElementById('run-sim-btn').addEventListener('click', runSimulation);
    }

    function runSimulation() {
        var resultEl = document.getElementById('sim-result');
        resultEl.innerHTML = '<div class="loading">正在运行菲克第二定律扩散模拟...</div>';

        var qs = 'material=' + encodeURIComponent(params.material) +
            '&temperature=' + params.temperature +
            '&humidity=' + params.humidity +
            '&porosity=' + params.porosity +
            '&concentration=' + params.concentration +
            '&hours=' + params.hours;

        fetch(App.apiBase + '/api/protection/simulate?' + qs)
            .then(function (r) { return r.json(); })
            .then(function (resp) {
                if (resp && resp.success && resp.data) {
                    renderResult(resp.data);
                } else {
                    resultEl.innerHTML = '<div class="error">模拟失败</div>';
                }
            })
            .catch(function () {
                resultEl.innerHTML = '<div class="error">网络错误</div>';
            });
    }

    function renderResult(data) {
        var resultEl = document.getElementById('sim-result');
        var efficiencyPct = (data.protection_efficiency * 100).toFixed(1);
        var effClass = data.protection_efficiency > 0.8 ? 'good' : data.protection_efficiency > 0.6 ? 'medium' : 'poor';

        var html = '';
        html += '<div class="sim-summary">';
        html += '<div class="summary-item"><span class="si-label">处理材料</span><span class="si-value">' + data.material_name + '</span></div>';
        html += '<div class="summary-item"><span class="si-label">平均渗透深度</span><span class="si-value">' + data.average_penetration_um.toFixed(2) + ' μm</span></div>';
        html += '<div class="summary-item"><span class="si-label">最大渗透深度</span><span class="si-value">' + data.max_penetration_um.toFixed(2) + ' μm</span></div>';
        html += '<div class="summary-item"><span class="si-label">有效扩散系数</span><span class="si-value">' + data.effective_diffusion_coeff.toExponential(2) + ' m²/s</span></div>';
        html += '<div class="summary-item"><span class="si-label">防护效率</span><span class="si-value eff ' + effClass + '">' + efficiencyPct + '%</span></div>';
        html += '<div class="summary-item"><span class="si-label">预计保护寿命</span><span class="si-value">' + data.estimated_lifetime_years.toFixed(1) + ' 年</span></div>';
        html += '</div>';

        html += '<div class="chart-wrapper">';
        html += '<h4>浓度分布剖面 (菲克第二定律)</h4>';
        html += '<canvas id="profile-canvas" width="600" height="250"></canvas>';
        html += '</div>';

        html += '<div class="chart-wrapper">';
        html += '<h4>渗透深度随时间变化</h4>';
        html += '<canvas id="time-canvas" width="600" height="220"></canvas>';
        html += '</div>';

        resultEl.innerHTML = html;

        drawProfileChart(data.profile, data.surface_concentration);
        drawTimeChart(data.time_series);
    }

    function drawProfileChart(profile, surfaceC) {
        var canvas = document.getElementById('profile-canvas');
        if (!canvas) return;
        var ctx = canvas.getContext('2d');
        var w = canvas.width, h = canvas.height;
        var padL = 60, padR = 20, padT = 20, padB = 40;
        var pw = w - padL - padR, ph = h - padT - padB;
        ctx.clearRect(0, 0, w, h);

        var maxDepth = 0;
        var maxC = 0;
        for (var i = 0; i < profile.length; i++) {
            maxDepth = Math.max(maxDepth, profile[i].depth_um);
            maxC = Math.max(maxC, profile[i].concentration_ratio);
        }
        maxDepth = Math.ceil(maxDepth / 100) * 100 || 500;

        ctx.strokeStyle = '#ddd';
        ctx.lineWidth = 1;
        for (var t = 0; t <= 4; t++) {
            var y = padT + ph * t / 4;
            ctx.beginPath(); ctx.moveTo(padL, y); ctx.lineTo(w - padR, y); ctx.stroke();
            var ratio = (1 - t / 4) * maxC;
            ctx.fillStyle = '#666'; ctx.font = '11px sans-serif'; ctx.textAlign = 'right';
            ctx.fillText(ratio.toFixed(2), padL - 5, y + 4);
        }
        for (var x = 0; x <= 5; x++) {
            var xp = padL + pw * x / 5;
            ctx.beginPath(); ctx.moveTo(xp, padT); ctx.lineTo(xp, padT + ph); ctx.stroke();
            ctx.fillStyle = '#666'; ctx.textAlign = 'center';
            ctx.fillText((maxDepth * x / 5).toFixed(0) + 'μm', xp, padT + ph + 18);
        }

        ctx.strokeStyle = '#e74c3c'; ctx.lineWidth = 2; ctx.beginPath();
        for (var j = 0; j < profile.length; j++) {
            var px = padL + pw * (profile[j].depth_um / maxDepth);
            var py = padT + ph * (1 - profile[j].concentration_ratio / Math.max(maxC, 0.01));
            if (j === 0) ctx.moveTo(px, py); else ctx.lineTo(px, py);
        }
        ctx.stroke();

        var grad = ctx.createLinearGradient(0, padT, 0, padT + ph);
        grad.addColorStop(0, 'rgba(231,76,60,0.35)');
        grad.addColorStop(1, 'rgba(231,76,60,0.02)');
        ctx.fillStyle = grad;
        ctx.lineTo(padL + pw * (profile[profile.length - 1].depth_um / maxDepth), padT + ph);
        ctx.lineTo(padL, padT + ph);
        ctx.closePath(); ctx.fill();

        ctx.fillStyle = '#333'; ctx.textAlign = 'center'; ctx.font = '12px sans-serif';
        ctx.fillText('渗透深度 (μm)', w / 2, h - 5);
        ctx.save(); ctx.translate(14, h / 2); ctx.rotate(-Math.PI / 2);
        ctx.fillText('归一化浓度 C/C₀', 0, 0); ctx.restore();
    }

    function drawTimeChart(timeSeries) {
        var canvas = document.getElementById('time-canvas');
        if (!canvas) return;
        var ctx = canvas.getContext('2d');
        var w = canvas.width, h = canvas.height;
        var padL = 60, padR = 30, padT = 20, padB = 40;
        var pw = w - padL - padR, ph = h - padT - padB;
        ctx.clearRect(0, 0, w, h);

        var maxT = 0, maxP = 0;
        for (var i = 0; i < timeSeries.length; i++) {
            maxT = Math.max(maxT, timeSeries[i].time_hours);
            maxP = Math.max(maxP, timeSeries[i].max_penetration_um);
        }
        maxP = Math.ceil(maxP / 50) * 50 || 200;

        ctx.strokeStyle = '#ddd';
        for (var t = 0; t <= 4; t++) {
            var y = padT + ph * t / 4;
            ctx.beginPath(); ctx.moveTo(padL, y); ctx.lineTo(w - padR, y); ctx.stroke();
            ctx.fillStyle = '#666'; ctx.font = '11px sans-serif'; ctx.textAlign = 'right';
            ctx.fillText((maxP * (1 - t / 4)).toFixed(0) + 'μm', padL - 5, y + 4);
        }
        for (var x = 0; x <= 5; x++) {
            var xp = padL + pw * x / 5;
            ctx.beginPath(); ctx.moveTo(xp, padT); ctx.lineTo(xp, padT + ph); ctx.stroke();
            ctx.fillStyle = '#666'; ctx.textAlign = 'center';
            ctx.fillText((maxT * x / 5).toFixed(0) + 'h', xp, padT + ph + 18);
        }

        ctx.strokeStyle = '#3498db'; ctx.lineWidth = 2; ctx.beginPath();
        for (var j = 0; j < timeSeries.length; j++) {
            var px = padL + pw * (timeSeries[j].time_hours / Math.max(maxT, 1));
            var py = padT + ph * (1 - timeSeries[j].max_penetration_um / Math.max(maxP, 1));
            if (j === 0) ctx.moveTo(px, py); else ctx.lineTo(px, py);
        }
        ctx.stroke();

        ctx.strokeStyle = '#27ae60'; ctx.lineWidth = 2; ctx.setLineDash([5, 3]); ctx.beginPath();
        for (var k = 0; k < timeSeries.length; k++) {
            var px2 = padL + pw * (timeSeries[k].time_hours / Math.max(maxT, 1));
            var py2 = padT + ph * (1 - timeSeries[k].avg_penetration_um / Math.max(maxP, 1));
            if (k === 0) ctx.moveTo(px2, py2); else ctx.lineTo(px2, py2);
        }
        ctx.stroke(); ctx.setLineDash([]);

        ctx.fillStyle = '#3498db'; ctx.fillRect(w - padR - 120, padT, 10, 10);
        ctx.fillStyle = '#333'; ctx.font = '11px sans-serif'; ctx.textAlign = 'left';
        ctx.fillText('最大渗透', w - padR - 105, padT + 9);
        ctx.fillStyle = '#27ae60'; ctx.fillRect(w - padR - 120, padT + 18, 10, 10);
        ctx.fillStyle = '#333'; ctx.fillText('平均渗透', w - padR - 105, padT + 27);

        ctx.fillStyle = '#333'; ctx.textAlign = 'center'; ctx.font = '12px sans-serif';
        ctx.fillText('时间 (小时)', w / 2, h - 5);
    }

    return {
        init: init
    };
})();
