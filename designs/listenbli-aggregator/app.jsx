// app.jsx — canvas composition.
//
// The canvas is the deliverable: the three directions sit side by side at
// 1440×900 so they can be compared on the same content, with the same
// "合并重复" control, and the same six capabilities on screen.
//
// `?only=a|b|c` renders one direction full-bleed at exact size — useful for a
// defect-free screenshot and for looking at a single direction without the
// canvas chrome.

const AGG_ONLY = new URLSearchParams(location.search).get("only");

// ---------------------------------------------------------------------------
// Comparison table — the legend that makes the three artboards decidable.
// ---------------------------------------------------------------------------

const CMP_ROWS = [
  ["聚合的隐喻", "多路输入 → 一路监听", "多份馆藏 → 一部总索引", "多条支流 → 一条主流"],
  ["来源怎么标识", "通道字母 A / B + 一颗 LED", "钞记 B 站 / YT，只有墨色粗细", "两字母标记 + 一道极淡的光晕"],
  ["品牌色的处置", "完全不用，只靠字母", "只留朱红一种，且只标「当前」", "稀释成色温，不参与信息层级"],
  ["主题", "深色 · 暖石墨 + 琥珀", "浅色 · 纸白 + 墨 + 朱红", "深色 · 蓝黑 + 冷青 + 暖沙"],
  ["字体", "Archivo Narrow / IBM Plex Mono", "Newsreader / Courier Prime", "Bricolage Grotesque / Spline Mono"],
  ["重复项的表达", "一行，两枚通道标签 + 竖括号", "一条著录，两枚出处章 + 脚注", "只留一条，行尾一个合并记号"],
  ["缓存的表达", "每通道一根缓冲电平条", "「已下载」栏位与印数", "用深度：已缓存的行「沉」得更实"],
  ["最强的一点", "聚合 = 调音台，语义零解释成本", "反类型；信息密度与可读性最好", "最有氛围；状态用空间而非标签"],
  ["最大的风险", "偏硬件，可能不够「音乐」", "浅色播放器需要用户接受", "氛围易失控，必须克制"],
];

function CompareTable() {
  return (
    <div className="cmp">
      <div className="cmp__head">
        <div className="cmp__hcell cmp__hcell--lead">维度</div>
        <div className="cmp__hcell">
          <b>A · 混音台</b>
          <span>Switchboard · 工业仪器</span>
        </div>
        <div className="cmp__hcell">
          <b>B · 索引</b>
          <span>The Index · 编辑印刷</span>
        </div>
        <div className="cmp__hcell">
          <b>C · 潮汐线</b>
          <span>Tideline · 有机空间</span>
        </div>
      </div>
      {CMP_ROWS.map((row) => (
        <div className="cmp__row" key={row[0]}>
          <div className="cmp__cell cmp__cell--lead">{row[0]}</div>
          <div className="cmp__cell">{row[1]}</div>
          <div className="cmp__cell">{row[2]}</div>
          <div className="cmp__cell">{row[3]}</div>
        </div>
      ))}
      <div className="cmp__foot">
        三个方向都遵守同一条红线：<b>来源不能变成商标色大乱斗</b>。也都在同一屏里交代了六件事 ——
        跨源去重、来源标识、统一队列、歌词出处、缓存状态、以及左下角那个「接入来源」的连接位。
      </div>
    </div>
  );
}

// ---------------------------------------------------------------------------

function AggregatorCanvas() {
  return (
    <DesignCanvas>
      <DCSection
        id="directions"
        title="聚合形态的三个方向"
        subtitle="同一份跨源搜索结果、同一个「合并重复」开关、同一套六项能力。请横比，不要单看一个。点任意画板标题可全屏。"
        gap={56}
      >
        <DCArtboard id="a" label="A · 混音台 Switchboard" width={1440} height={900}>
          <DirectionA />
        </DCArtboard>
        <DCArtboard id="b" label="B · 索引 The Index" width={1440} height={900}>
          <DirectionB />
        </DCArtboard>
        <DCArtboard id="c" label="C · 潮汐线 Tideline" width={1440} height={900}>
          <DirectionC />
        </DCArtboard>
      </DCSection>

      <DCSection
        id="howto"
        title="怎么选"
        subtitle="三个方向的差别不在功能，而在「来源」这件事怎么被表达。"
      >
        <DCArtboard id="compare" label="方向对比表" width={1440} height={580}>
          <CompareTable />
        </DCArtboard>
      </DCSection>
    </DesignCanvas>
  );
}

function AggregatorApp() {
  if (AGG_ONLY === "a") return <div className="only"><DirectionA /></div>;
  if (AGG_ONLY === "b") return <div className="only"><DirectionB /></div>;
  if (AGG_ONLY === "c") return <div className="only"><DirectionC /></div>;
  if (AGG_ONLY === "compare") return <div className="only" style={{ height: 580 }}><CompareTable /></div>;
  return <AggregatorCanvas />;
}

ReactDOM.createRoot(document.getElementById("root")).render(<AggregatorApp />);
