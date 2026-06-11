var GroundwaterPanel = (function () {
    var container = null;
    var params = { days: 90, threshold: 100 };

    function init(el) {
        container = el;
        renderControls();
    }

    function renderControls() {
        if (!container) return;
        var html = '';
        html += '<div class="panel-header"><h3>遗址地下水动态预警与氯离子扩散模拟</h3></div>';
        html += '<div class="sim-controls">';
        html += '<div class="form-row"><label>模拟天数：</label><input type="range" id="gw-days" min="7" max="365" step="1" value="90"><span id="gw-days-val">90</span></div>';
        html += '<div class="form-row"><label>Cl⁻阈值(ppm)：</label><input type="range" id="gw-thresh" min="20" max="300" step="5" value="100"><span id="gw-thresh-val">100</span></div>';
        html += '<button class="btn-primary" id="gw-sim-btn">运行地下水模拟</button>';
        html += '</div>';
        html += '<div id="gw-result" class="sim-result"></div>';
        container.innerHTML = html;

        document.getElementById('gw-days').addEventListener('input', function (e) {
            document.getElementById('gw-days-val').textContent = e.target.value;
            params.days = parseInt(e.target.value);
        });
        document.getElementById('gw-thresh').addEventListener('input', function (e) {
            document.getElementById('gw-thresh-val').textContent = e.target.value;
            params.threshold = parseInt(e.target.value);
        });
        document.getElementById('gw-sim-btn').addEventListener('click', runSimulation);
    }

    function runSimulation() {
        var el = document.getElementById('gw-result');
        el.innerHTML = '<div class="loading">正在求解MODFLOW稳态流场 + 氯离子对流-扩散方程...</div>';
        var qs = 'days=' + params.days + '&threshold=' + params.threshold;
        fetch(App.apiBase + '/api/groundwater/simulate?' + qs)
            .then(function (r) { return r.json(); })
            .then(function (resp) {
                if (resp && resp.success && resp.data) {
                    renderResult(resp.data);
                } else {
                    el.innerHTML = '<div class="error">模拟失败</div>';
                }
            })
            .catch(function () {
                el.innerHTML = '<div class="error">网络错误</div>';
            });
    }

    function renderResult(data) {
        var el = document.getElementById('gw-result');
        var flow = data.flow_field;
        var diffusion = data.diffusion;
        var warn = diffusion.overall_warning;

        var warnClass = warn.warning_level === '严重' ? 'warn-critical' :
            warn.warning_level === '高' ? 'warn-high' :
                warn.warning_level === '中' ? 'warn-medium' :
                    warn.warning_level === '低' ? 'warn-low' : 'warn-none';

        var html = '';

        html += '<div class="gw-warning ' + warnClass + '">';
        html += '<div class="gw-warn-icon">⚠</div>';
        html += '<div class="gw-warn-body">';
        html += '<div class="gw-warn-title">地下水污染预警状态：<b>' + warn.warning_level + '</b></div>';
        if (warn.has_warning) {
            html += '<div class="gw-warn-desc">预计 ' + warn.time_to_first_impact_days.toFixed(1) + ' 天后污染物到达首个敏感区</div>';
            if (warn.affected_sensitive_zones && warn.affected_sensitive_zones.length > 0) {
                html += '<div class="gw-warn-zones">受影响区域：' + warn.affected_sensitive_zones.join('、') + '</div>';
            }
        } else {
            html += '<div class="gw-warn-desc">当前模拟周期内氯离子未到达敏感保护区</div>';
        }
        html += '</div>';
        html += '</div>';

        html += '<div class="gw-summary">';
        html += '<div class="summary-item"><span class="si-label">水力梯度</span><span class="si-value">' + flow.head_gradient.toFixed(4) + '</span></div>';
        html += '<div class="summary-item"><span class="si-label">平均渗流速度</span><span class="si-value">' + flow.avg_velocity_m_d.toFixed(3) + ' m/d</span></div>';
        html += '<div class="summary-item"><span class="si-label">最大渗流速度</span><span class="si-value">' + flow.max_velocity_m_d.toFixed(3) + ' m/d</span></div>';
        html += '<div class="summary-item"><span class="si-label">平均水头</span><span class="si-value">' + flow.avg_head_m.toFixed(2) + ' m</span></div>';
        html += '<div class="summary-item"><span class="si-label">模拟周期</span><span class="si-value">' + diffusion.total_simulation_days.toFixed(0) + ' 天</span></div>';
        html += '<div class="summary-item"><span class="si-label">求解收敛</span><span class="si-value">' + (flow.convergence_status ? '✓' : '✗') + ' (' + flow.iterations + ' 迭代)</span></div>';
        html += '</div>';

        if (flow.travel_time_days && flow.travel_time_days.length > 0) {
            html += '<div class="travel-times">';
            html += '<h4>污染羽运移时间估算</h4>';
            html += '<table class="data-table"><thead><tr><th>污染源</th><th>到达监测井</th><th>距离(m)</th><th>运移时间(天)</th></tr></thead><tbody>';
            for (var t = 0; t < flow.travel_time_days.length; t++) {
                var tt = flow.travel_time_days[t];
                html += '<tr><td>' + tt.source_id + '</td><td>' + tt.target_id + '</td><td>' + tt.distance_m.toFixed(1) + '</td><td>' + (tt.travel_days === Infinity ? '∞' : tt.travel_days.toFixed(1)) + '</td></tr>';
            }
            html += '</tbody></table></div>';
        }

        if (diffusion.time_series && diffusion.time_series.length > 0) {
            html += '<div class="chart-wrapper">';
            html += '<h4>污染羽扩散时间序列</h4>';
            html += '<canvas id="plume-canvas" width="600" height="240"></canvas>';
            html += '</div>';
        }

        html += '<div class="chart-wrapper">';
        html += '<h4>最终浓度网格分布</h4>';
        html += '<canvas id="conc-canvas" width="600" height="420"></canvas>';
        html += '</div>';

        if (diffusion.contamination_paths && diffusion.contamination_paths.length > 0) {
            html += '<div class="contam-paths">';
            html += '<h4>主要污染扩散路径追踪</h4>';
            for (var p = 0; p < diffusion.contamination_paths.length; p++) {
                var path = diffusion.contamination_paths[p];
                html += '<div class="path-card">';
                html += '<div class="path-header">路径 ' + (p + 1) + ' 源: ' + path.source_id +
                    ' | 最大浓度: ' + path.max_concentration_ppm.toFixed(1) + ' ppm' +
                    ' | 风险: ' + (path.risk_level.as_str || path.risk_level) + '</div>';
                if (path.arrival_time_to_sensitive_zones && path.arrival_time_to_sensitive_zones.length > 0) {
                    html += '<div class="path-alerts">';
                    for (var a = 0; a < path.arrival_time_to_sensitive_zones.length; a++) {
                        var arr = path.arrival_time_to_sensitive_zones[a];
                        html += '<span class="path-alert-tag">到达 ' + arr.zone_name + ': ' + arr.arrival_days.toFixed(1) + ' 天 (峰值 ' + arr.peak_ppm.toFixed(0) + ' ppm)</span>';
                    }
                    html += '</div>';
                }
                html += '</div>';
            }
            html += '</div>';
        }

        if (warn.mitigation_suggestions && warn.mitigation_suggestions.length > 0) {
            html += '<div class="recommendations">';
            html += '<h4>地下水污染防控建议</h4><ul>';
            for (var r = 0; r < warn.mitigation_suggestions.length; r++) {
                html += '<li>' + warn.mitigation_suggestions[r] + '</li>';
            }
            html += '</ul></div>';
        }

        el.innerHTML = html;

        if (diffusion.time_series) drawPlumeChart(diffusion.time_series);
        if (flow && diffusion.final_concentration_grid) drawConcentrationGrid(flow, diffusion.final_concentration_grid, diffusion.sensitive_zones, params.threshold);
    }

    function drawPlumeChart(series) {
        var canvas = document.getElementById('plume-canvas');
        if (!canvas) return;
        var ctx = canvas.getContext('2d');
        var w = canvas.width, h = canvas.height;
        var padL = 60, padR = 80, padT = 20, padB = 40;
        var pw = w - padL - padR, ph = h - padT - padB;
        ctx.clearRect(0, 0, w, h);

        var maxT = 0, maxC = 0, maxA = 0;
        for (var i = 0; i < series.length; i++) {
            maxT = Math.max(maxT, series[i].time_days);
            maxC = Math.max(maxC, series[i].max_concentration_ppm);
            maxA = Math.max(maxA, series[i].affected_cells);
        }
        maxC = Math.ceil(maxC / 50) * 50 || 200;

        ctx.strokeStyle = '#ddd';
        for (var t = 0; t <= 4; t++) {
            var y = padT + ph * t / 4;
            ctx.beginPath(); ctx.moveTo(padL, y); ctx.lineTo(w - padR, y); ctx.stroke();
            ctx.fillStyle = '#666'; ctx.font = '11px sans-serif'; ctx.textAlign = 'right';
            ctx.fillText((maxC * (1 - t / 4)).toFixed(0) + 'ppm', padL - 5, y + 4);
        }
        for (var x = 0; x <= 5; x++) {
            var xp = padL + pw * x / 5;
            ctx.beginPath(); ctx.moveTo(xp, padT); ctx.lineTo(xp, padT + ph); ctx.stroke();
            ctx.fillStyle = '#666'; ctx.textAlign = 'center';
            ctx.fillText((maxT * x / 5).toFixed(0) + 'd', xp, padT + ph + 18);
        }

        ctx.strokeStyle = '#e74c3c'; ctx.lineWidth = 2; ctx.beginPath();
        for (var j = 0; j < series.length; j++) {
            var px = padL + pw * (series[j].time_days / Math.max(maxT, 1));
            var py = padT + ph * (1 - series[j].max_concentration_ppm / Math.max(maxC, 1));
            if (j === 0) ctx.moveTo(px, py); else ctx.lineTo(px, py);
        }
        ctx.stroke();

        ctx.save();
        ctx.strokeStyle = '#3498db'; ctx.lineWidth = 1.5;
        ctx.setLineDash([4, 3]);
        var rightY0 = padT + ph * (1 - params.threshold / Math.max(maxC, 1));
        ctx.beginPath(); ctx.moveTo(padL, rightY0); ctx.lineTo(w - padR, rightY0); ctx.stroke();
        ctx.setLineDash([]);
        ctx.fillStyle = '#3498db'; ctx.textAlign = 'left'; ctx.font = '11px sans-serif';
        ctx.fillText('阈值 ' + params.threshold + 'ppm', padL + 5, rightY0 - 5);
        ctx.restore();

        ctx.fillStyle = '#e74c3c'; ctx.fillRect(w - padR + 10, padT, 10, 10);
        ctx.fillStyle = '#333'; ctx.font = '11px sans-serif'; ctx.textAlign = 'left';
        ctx.fillText('最大Cl⁻浓度', w - padR + 25, padT + 9);

        ctx.fillStyle = '#333'; ctx.textAlign = 'center'; ctx.font = '12px sans-serif';
        ctx.fillText('时间 (天)', w / 2, h - 5);
    }

    function drawConcentrationGrid(flow, grid, zones, threshold) {
        var canvas = document.getElementById('conc-canvas');
        if (!canvas) return;
        var ctx = canvas.getContext('2d');
        var w = canvas.width, h = canvas.height;
        var padL = 40, padR = 110, padT = 20, padB = 40;
        var pw = w - padL - padR, ph = h - padT - padB;
        ctx.clearRect(0, 0, w, h);

        var rows = flow.grid_rows, cols = flow.grid_cols;
        var cellW = pw / cols, cellH = ph / rows;

        var maxC = 0;
        for (var g = 0; g < grid.length; g++) maxC = Math.max(maxC, grid[g].concentration_ppm);
        maxC = Math.max(maxC, threshold * 1.5);

        for (var r = 0; r < rows; r++) {
            for (var c = 0; c < cols; c++) {
                var idx = r * cols + c;
                if (idx >= grid.length) continue;
                var cell = grid[idx];
                var ratio = Math.min(cell.concentration_ppm / maxC, 1);
                var color = concentrationColor(ratio);
                ctx.fillStyle = color;
                ctx.fillRect(padL + c * cellW, padT + r * cellH, cellW + 0.5, cellH + 0.5);
                if (cell.exceed_threshold) {
                    ctx.strokeStyle = 'rgba(255,0,0,0.5)';
                    ctx.lineWidth = 1;
                    ctx.strokeRect(padL + c * cellW + 1, padT + r * cellH + 1, cellW - 2, cellH - 2);
                }
            }
        }

        if (flow.wells) {
            for (var wk = 0; wk < flow.wells.length; wk++) {
                var well = flow.wells[wk];
                var wx = padL + well.col * cellW + cellW / 2;
                var wy = padT + well.row * cellH + cellH / 2;
                var color = well.well_type === '污染源' || well.well_type === 'ContaminationSource' ? '#e74c3c' :
                    well.well_type === '监测井' || well.well_type === 'Monitor' ? '#3498db' :
                        well.well_type === '抽水井' || well.well_type === 'Pumping' ? '#9b59b6' : '#27ae60';
                ctx.beginPath(); ctx.arc(wx, wy, 7, 0, Math.PI * 2);
                ctx.fillStyle = color; ctx.fill();
                ctx.strokeStyle = '#fff'; ctx.lineWidth = 2; ctx.stroke();
                ctx.fillStyle = '#fff'; ctx.font = 'bold 9px sans-serif'; ctx.textAlign = 'center'; ctx.textBaseline = 'middle';
                ctx.fillText(wk + 1, wx, wy);
            }
        }

        if (zones) {
            for (var z = 0; z < zones.length; z++) {
                var zone = zones[z];
                var zx = padL + (zone.x_center / (flow.cell_size_m * cols)) * pw;
                var zy = padT + (zone.y_center / (flow.cell_size_m * rows)) * ph;
                var zr = (zone.radius_m / (flow.cell_size_m * Math.min(cols, rows))) * Math.min(pw, ph);
                ctx.beginPath(); ctx.arc(zx, zy, zr, 0, Math.PI * 2);
                ctx.fillStyle = 'rgba(46,204,113,0.18)';
                ctx.fill(); ctx.strokeStyle = '#27ae60'; ctx.lineWidth = 1.5; ctx.setLineDash([4, 3]);
                ctx.stroke(); ctx.setLineDash([]);
                ctx.fillStyle = '#27ae60'; ctx.font = '10px sans-serif'; ctx.textAlign = 'center';
                ctx.fillText(zone.name.substring(0, 6), zx, zy - zr - 5);
            }
        }

        var legendX = w - padR + 10;
        for (var li = 0; li <= 10; li++) {
            var ly = padT + ph * (1 - li / 10);
            ctx.fillStyle = concentrationColor(li / 10);
            ctx.fillRect(legendX, ly, 18, ph / 10 + 1);
        }
        ctx.strokeStyle = '#333'; ctx.strokeRect(legendX, padT, 18, ph);
        ctx.fillStyle = '#333'; ctx.textAlign = 'left'; ctx.font = '11px sans-serif';
        ctx.fillText(maxC.toFixed(0) + 'ppm', legendX + 22, padT + 4);
        ctx.fillText('0ppm', legendX + 22, padT + ph);
        ctx.fillText('Cl⁻浓度', legendX + 5, padT - 5);

        ctx.fillStyle = '#333'; ctx.textAlign = 'center'; ctx.font = '12px sans-serif';
        ctx.fillText('距离 (m)', padL + pw / 2, h - 5);
    }

    function concentrationColor(ratio) {
        ratio = Math.max(0, Math.min(1, ratio));
        if (ratio < 0.2) {
            var t = ratio / 0.2;
            return 'rgb(' + Math.round(39 + (241 - 39) * t) + ',' +
                Math.round(174 + (196 - 174) * t) + ',' +
                Math.round(96 + (15 - 96) * t) + ')';
        } else if (ratio < 0.5) {
            var t = (ratio - 0.2) / 0.3;
            return 'rgb(' + Math.round(241 + (230 - 241) * t) + ',' +
                Math.round(196 + (126 - 196) * t) + ',' +
                Math.round(15 + (34 - 15) * t) + ')';
        } else if (ratio < 0.8) {
            var t = (ratio - 0.5) / 0.3;
            return 'rgb(' + Math.round(230 + (231 - 230) * t) + ',' +
                Math.round(126 + (76 - 126) * t) + ',' +
                Math.round(34 + (60 - 34) * t) + ')';
        } else {
            var t = (ratio - 0.8) / 0.2;
            return 'rgb(' + Math.round(231 + (100 - 231) * t) + ',' +
                Math.round(76 + (0 - 76) * t) + ',' +
                Math.round(60 + (0 - 60) * t) + ')';
        }
    }

    return {
        init: init
    };
})();
