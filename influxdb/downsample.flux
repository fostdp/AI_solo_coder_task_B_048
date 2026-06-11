// ============================================================
// 考古腐蚀监测系统 - Flux 降采样任务集合 (InfluxDB 2.x)
// 组织: archaeology
// 
// 说明: 以下是三个独立的 Flux 任务配置。
// 每个任务需要分别通过 InfluxDB UI 或 CLI 单独创建。
// 
// Bucket 保留策略:
//   - corrosion_data (原始): 30天
//   - corrosion_data_downsampled_1m: 90天
//   - corrosion_data_downsampled_1h: 365天
//   - corrosion_data_downsampled_1d: 3年 (1095天)
// ============================================================

// ============================================================
// 任务 1: 1分钟降采样 (downsample_1m)
// 执行频率: 每 1 分钟
// 偏移: 30 秒 (等待数据写入完成)
// 数据范围: 过去 5 分钟 (防止数据延迟丢失)
// 聚合函数: mean (平均值)
// 包含测量: metal_corrosion, soil_environment
// ============================================================

/*
// --- 任务配置 ---
option task = {
    name: "downsample_1m",
    every: 1m,
    offset: 30s
}

// --- 腐蚀数据 1分钟降采样 ---
corrosion = from(bucket: "corrosion_data")
    |> range(start: -5m)
    |> filter(fn: (r) => r._measurement == "metal_corrosion")
    |> aggregateWindow(
        every: 1m,
        fn: mean,
        createEmpty: false
    )
    |> to(bucket: "corrosion_data_downsampled_1m", org: "archaeology")

// --- 土壤环境数据 1分钟降采样 ---
soil = from(bucket: "corrosion_data")
    |> range(start: -5m)
    |> filter(fn: (r) => r._measurement == "soil_environment")
    |> aggregateWindow(
        every: 1m,
        fn: mean,
        createEmpty: false
    )
    |> to(bucket: "corrosion_data_downsampled_1m", org: "archaeology")
*/

// ============================================================
// 任务 2: 1小时降采样 (downsample_1h)
// 执行频率: 每 1 小时
// 偏移: 5 分钟
// 数据范围: 过去 2 小时
// 聚合函数: mean, median, stddev
// 包含测量: metal_corrosion, soil_environment
// 注意: 从 1分钟 降采样数据进一步聚合
// ============================================================

/*
// --- 任务配置 ---
option task = {
    name: "downsample_1h",
    every: 1h,
    offset: 5m
}

// --- 腐蚀数据 - 均值 ---
corrosion_mean = from(bucket: "corrosion_data_downsampled_1m")
    |> range(start: -2h)
    |> filter(fn: (r) => r._measurement == "metal_corrosion")
    |> aggregateWindow(
        every: 1h,
        fn: mean,
        createEmpty: false
    )
    |> map(fn: (r) => ({r with _field: "mean_" + r._field}))
    |> to(bucket: "corrosion_data_downsampled_1h", org: "archaeology")

// --- 腐蚀数据 - 中位数 ---
corrosion_median = from(bucket: "corrosion_data_downsampled_1m")
    |> range(start: -2h)
    |> filter(fn: (r) => r._measurement == "metal_corrosion")
    |> aggregateWindow(
        every: 1h,
        fn: median,
        createEmpty: false
    )
    |> map(fn: (r) => ({r with _field: "median_" + r._field}))
    |> to(bucket: "corrosion_data_downsampled_1h", org: "archaeology")

// --- 腐蚀数据 - 标准差 ---
corrosion_stddev = from(bucket: "corrosion_data_downsampled_1m")
    |> range(start: -2h)
    |> filter(fn: (r) => r._measurement == "metal_corrosion")
    |> aggregateWindow(
        every: 1h,
        fn: stddev,
        createEmpty: false
    )
    |> map(fn: (r) => ({r with _field: "stddev_" + r._field}))
    |> to(bucket: "corrosion_data_downsampled_1h", org: "archaeology")

// --- 土壤环境数据 - 均值 ---
soil_mean = from(bucket: "corrosion_data_downsampled_1m")
    |> range(start: -2h)
    |> filter(fn: (r) => r._measurement == "soil_environment")
    |> aggregateWindow(
        every: 1h,
        fn: mean,
        createEmpty: false
    )
    |> map(fn: (r) => ({r with _field: "mean_" + r._field}))
    |> to(bucket: "corrosion_data_downsampled_1h", org: "archaeology")

// --- 土壤环境数据 - 中位数 ---
soil_median = from(bucket: "corrosion_data_downsampled_1m")
    |> range(start: -2h)
    |> filter(fn: (r) => r._measurement == "soil_environment")
    |> aggregateWindow(
        every: 1h,
        fn: median,
        createEmpty: false
    )
    |> map(fn: (r) => ({r with _field: "median_" + r._field}))
    |> to(bucket: "corrosion_data_downsampled_1h", org: "archaeology")

// --- 土壤环境数据 - 标准差 ---
soil_stddev = from(bucket: "corrosion_data_downsampled_1m")
    |> range(start: -2h)
    |> filter(fn: (r) => r._measurement == "soil_environment")
    |> aggregateWindow(
        every: 1h,
        fn: stddev,
        createEmpty: false
    )
    |> map(fn: (r) => ({r with _field: "stddev_" + r._field}))
    |> to(bucket: "corrosion_data_downsampled_1h", org: "archaeology")
*/

// ============================================================
// 任务 3: 每日聚合 (downsample_1d)
// 执行频率: 每 1 天
// 偏移: 10 分钟
// 数据范围: 过去 48 小时
// 聚合函数: mean, min, max
// 包含测量: metal_corrosion, soil_environment
// 注意: 从 1小时 降采样数据进一步聚合
// ============================================================

/*
// --- 任务配置 ---
option task = {
    name: "downsample_1d",
    every: 1d,
    offset: 10m
}

// --- 腐蚀数据 - 均值 ---
corrosion_mean = from(bucket: "corrosion_data_downsampled_1h")
    |> range(start: -48h)
    |> filter(fn: (r) => r._measurement == "metal_corrosion" and r._field =~ /^mean_/)
    |> aggregateWindow(
        every: 1d,
        fn: mean,
        createEmpty: false
    )
    |> map(fn: (r) => ({r with _field: "daily_mean_" + r._field}))
    |> to(bucket: "corrosion_data_downsampled_1d", org: "archaeology")

// --- 腐蚀数据 - 最小值 ---
corrosion_min = from(bucket: "corrosion_data_downsampled_1h")
    |> range(start: -48h)
    |> filter(fn: (r) => r._measurement == "metal_corrosion" and r._field =~ /^mean_/)
    |> aggregateWindow(
        every: 1d,
        fn: min,
        createEmpty: false
    )
    |> map(fn: (r) => ({r with _field: "daily_min_" + r._field}))
    |> to(bucket: "corrosion_data_downsampled_1d", org: "archaeology")

// --- 腐蚀数据 - 最大值 ---
corrosion_max = from(bucket: "corrosion_data_downsampled_1h")
    |> range(start: -48h)
    |> filter(fn: (r) => r._measurement == "metal_corrosion" and r._field =~ /^mean_/)
    |> aggregateWindow(
        every: 1d,
        fn: max,
        createEmpty: false
    )
    |> map(fn: (r) => ({r with _field: "daily_max_" + r._field}))
    |> to(bucket: "corrosion_data_downsampled_1d", org: "archaeology")

// --- 土壤环境数据 - 均值 ---
soil_mean = from(bucket: "corrosion_data_downsampled_1h")
    |> range(start: -48h)
    |> filter(fn: (r) => r._measurement == "soil_environment" and r._field =~ /^mean_/)
    |> aggregateWindow(
        every: 1d,
        fn: mean,
        createEmpty: false
    )
    |> map(fn: (r) => ({r with _field: "daily_mean_" + r._field}))
    |> to(bucket: "corrosion_data_downsampled_1d", org: "archaeology")

// --- 土壤环境数据 - 最小值 ---
soil_min = from(bucket: "corrosion_data_downsampled_1h")
    |> range(start: -48h)
    |> filter(fn: (r) => r._measurement == "soil_environment" and r._field =~ /^mean_/)
    |> aggregateWindow(
        every: 1d,
        fn: min,
        createEmpty: false
    )
    |> map(fn: (r) => ({r with _field: "daily_min_" + r._field}))
    |> to(bucket: "corrosion_data_downsampled_1d", org: "archaeology")

// --- 土壤环境数据 - 最大值 ---
soil_max = from(bucket: "corrosion_data_downsampled_1h")
    |> range(start: -48h)
    |> filter(fn: (r) => r._measurement == "soil_environment" and r._field =~ /^mean_/)
    |> aggregateWindow(
        every: 1d,
        fn: max,
        createEmpty: false
    )
    |> map(fn: (r) => ({r with _field: "daily_max_" + r._field}))
    |> to(bucket: "corrosion_data_downsampled_1d", org: "archaeology")
*/

// ============================================================
// 使用说明
// ============================================================
//
// 方法 1: 通过 InfluxDB UI 创建任务
//   1. 打开 InfluxDB UI (http://localhost:8086)
//   2. 进入 Tasks (任务) 页面
//   3. 点击 "Create Task" -> "New Task"
//   4. 复制对应任务的 Flux 代码 (去掉 /* */ 注释)
//   5. 设置任务名称和执行频率
//   6. 点击保存
//
// 方法 2: 通过 Influx CLI 创建任务
//   influx task create --name "downsample_1m" --file task_1m.flux
//
// 方法 3: 通过 API 创建
//   POST /api/v2/tasks
//   Body: { "flux": "...", "org": "archaeology" }
//
// ============================================================
//
// 相关 Bucket 创建命令 (需要先创建这些 bucket):
//
//   influx bucket create \
//     --name corrosion_data_downsampled_1m \
//     --org archaeology \
//     --retention 90d
//
//   influx bucket create \
//     --name corrosion_data_downsampled_1h \
//     --org archaeology \
//     --retention 365d
//
//   influx bucket create \
//     --name corrosion_data_downsampled_1d \
//     --org archaeology \
//     --retention 1095d
//
// ============================================================
