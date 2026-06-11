var MicrobiomePanel = (function () {
    var container = null;

    function init(el) {
        container = el;
    }

    function load() {
        if (!container) return;
        container.innerHTML = '<div class="loading">正在加载微生物测序分析...</div>';
        Promise.all([
            fetch(App.apiBase + '/api/microbiome/samples').then(function (r) { return r.json(); }),
            fetch(App.apiBase + '/api/microbiome/analysis').then(function (r) { return r.json(); })
        ]).then(function (results) {
            var samples = results[0] && results[0].data ? results[0].data : [];
            var analysis = results[1] && results[1].data ? results[1].data : null;
            render(samples, analysis);
        }).catch(function () {
            container.innerHTML = '<div class="error">加载失败</div>';
        });
    }

    function render(samples, analysis) {
        if (!container) return;
        var html = '';
        html += '<div class="panel-header"><h3>土壤微生物活性与腐蚀关联分析</h3></div>';

        if (analysis) {
            var riskClass = analysis.risk_level === '高' ? 'risk-high' :
                analysis.risk_level === '较高' ? 'risk-medhigh' :
                    analysis.risk_level === '中等' ? 'risk-medium' : 'risk-low';
            html += '<div class="analysis-summary ' + riskClass + '">';
            html += '<div class="as-main">';
            html += '<div class="as-label">微生物腐蚀综合风险</div>';
            html += '<div class="as-value">' + analysis.risk_level + '</div>';
            html += '<div class="as-score">风险指数：' + (analysis.overall_microbiome_risk * 100).toFixed(1) + '</div>';
            html += '</div>';
            html += '<div class="as-side">';
            html += '<div class="as-item"><span>样本数</span><b>' + analysis.sample_count + '</b></div>';
            html += '<div class="as-item"><span>模型置信度</span><b>' + (analysis.model_confidence * 100).toFixed(0) + '%</b></div>';
            html += '<div class="as-item"><span>预测腐蚀速率</span><b>' + analysis.predicted_corrosion_rate.toFixed(4) + ' mm/y</b></div>';
            html += '<div class="as-item"><span>多样性相关性</span><b>' + analysis.diversity_correlation.toFixed(3) + '</b></div>';
            html += '</div>';
            html += '</div>';

            html += '<div class="correlation-summary">';
            html += '<div class="corr-item"><span class="corr-label">生物量×腐蚀</span>';
            html += corrBar(analysis.biomass_correlation);
            html += '<span class="corr-val">' + analysis.biomass_correlation.toFixed(2) + '</span></div>';
            html += '<div class="corr-item"><span class="corr-label">pH×微生物</span>';
            html += corrBar(analysis.ph_microbe_interaction);
            html += '<span class="corr-val">' + analysis.ph_microbe_interaction.toFixed(2) + '</span></div>';
            html += '<div class="corr-item"><span class="corr-label">Cl⁻×微生物</span>';
            html += corrBar(analysis.chloride_microbe_interaction);
            html += '<span class="corr-val">' + analysis.chloride_microbe_interaction.toFixed(2) + '</span></div>';
            html += '</div>';

            if (analysis.top_corrosion_promoters && analysis.top_corrosion_promoters.length > 0) {
                html += '<div class="key-factors">';
                html += '<h4>关键促腐蚀因子 (随机森林Gini重要度)</h4>';
                var topN = analysis.feature_importance.slice(0, 8);
                for (var i = 0; i < topN.length; i++) {
                    var f = topN[i];
                    html += '<div class="factor-row">';
                    html += '<span class="factor-rank">' + (i + 1) + '</span>';
                    html += '<span class="factor-name" title="' + f.description + '">' + f.feature_name + '</span>';
                    html += '<div class="factor-bar-bg"><div class="factor-bar-fill" style="width:' + (f.importance_score * 100).toFixed(1) + '%;background:' + (f.corrosion_effect.indexOf('促') >= 0 ? '#e74c3c' : '#27ae60') + '"></div></div>';
                    html += '<span class="factor-tag">' + f.corrosion_effect + '</span>';
                    html += '</div>';
                }
                html += '</div>';
            }

            if (analysis.gene_category_scores) {
                html += '<div class="gene-cat">';
                html += '<h4>功能基因类别 - 腐蚀相关活性</h4>';
                html += '<div class="gene-bars">';
                var catKeys = Object.keys(analysis.gene_category_scores);
                for (var c = 0; c < catKeys.length; c++) {
                    var score = analysis.gene_category_scores[catKeys[c]];
                    var color = score > 0.2 ? '#e74c3c' : score < -0.2 ? '#27ae60' : '#95a5a6';
                    html += '<div class="gene-col">';
                    html += '<div class="gene-bar">';
                    var posH = Math.max(score, 0) * 80;
                    var negH = Math.max(-score, 0) * 80;
                    html += '<div class="gene-bar-neg" style="height:' + negH + 'px"></div>';
                    html += '<div class="gene-bar-mid"></div>';
                    html += '<div class="gene-bar-pos" style="height:' + posH + 'px;background:' + color + '"></div>';
                    html += '</div>';
                    html += '<span class="gene-cat-label">' + catKeys[c] + '</span>';
                    html += '<span class="gene-cat-val">' + score.toFixed(2) + '</span>';
                    html += '</div>';
                }
                html += '</div>';
                html += '</div>';
            }

            if (analysis.risk_recommendations && analysis.risk_recommendations.length > 0) {
                html += '<div class="recommendations">';
                html += '<h4>微生物防治建议</h4><ul>';
                for (var r = 0; r < analysis.risk_recommendations.length; r++) {
                    html += '<li>' + analysis.risk_recommendations[r] + '</li>';
                }
                html += '</ul></div>';
            }
        }

        if (samples && samples.length > 0) {
            html += '<div class="sample-list">';
            html += '<h4>测序采样点</h4>';
            for (var s = 0; s < samples.length; s++) {
                var sm = samples[s];
                html += '<div class="sample-card">';
                html += '<div class="sc-header">';
                html += '<span class="sc-id">' + sm.sample_id + '</span>';
                html += '<span class="sc-zone">' + sm.zone + '</span>';
                html += '</div>';
                html += '<div class="sc-stats">';
                html += '<span>Shannon: ' + sm.shannon_diversity.toFixed(2) + '</span>';
                html += '<span>生物量: ' + (sm.microbial_biomass_cfu_g / 1e6).toFixed(2) + '×10⁶ CFU/g</span>';
                html += '<span>腐蚀率: ' + sm.corrosion_rate_observed.toFixed(3) + ' mm/y</span>';
                html += '<span>pH: ' + sm.ph.toFixed(2) + '</span>';
                html += '</div>';
                if (sm.taxa && sm.taxa.length > 0) {
                    html += '<div class="sc-taxa">';
                    html += '<span class="sc-taxa-label">优势菌门:</span>';
                    var top = sm.taxa.filter(function (t) { return t.taxon_rank === 'Phylum' || t.taxon_rank === '门'; })
                        .sort(function (a, b) { return b.relative_abundance - a.relative_abundance; })
                        .slice(0, 3);
                    for (var t = 0; t < top.length; t++) {
                        html += '<span class="taxon-tag">' + top[t].taxon_name + ' ' + top[t].relative_abundance.toFixed(1) + '%</span>';
                    }
                    html += '</div>';
                }
                html += '</div>';
            }
            html += '</div>';
        }

        container.innerHTML = html;
    }

    function corrBar(value) {
        var pct = Math.min(Math.abs(value), 1) * 100;
        var color = value > 0 ? '#e74c3c' : '#27ae60';
        return '<div class="corr-bar-bg"><div class="corr-bar-fill" style="width:' + pct.toFixed(0) + '%;background:' + color + '"></div></div>';
    }

    return {
        init: init,
        load: load
    };
})();
