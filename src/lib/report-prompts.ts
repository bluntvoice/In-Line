import type { StatisticsPreset } from "./statistics-range";

export type ReportTemplateMode="review"|"concise"|"byType"|"custom";

export const REPORT_TEMPLATE_OPTIONS:{value:ReportTemplateMode;label:string;description:string}[]=[
  {value:"review",label:"工作复盘型",description:"工作概览、重点完成、持续推进、问题风险和下期关注。"},
  {value:"concise",label:"精简汇报型",description:"用较短篇幅呈现概览、重点工作和后续关注。"},
  {value:"byType",label:"事项分类型",description:"按事项类型归纳数量、完成情况、代表事项和待跟进工作。"},
  {value:"custom",label:"自定义模板",description:"保留报告类型和日期占位符，其余结构与措辞由用户决定。"}
];

export const DEFAULT_CUSTOM_REPORT_TEMPLATE=`请使用 In Line MCP，根据真实事项和办理记录生成一份简洁的{{报告类型}}。

报告时间范围：{{开始日期}} 至 {{结束日期}}（包含结束日）。

请调用 get_report_summary 和 list_report_items 读取完整数据，并按照以下结构撰写：`;

const DATA_INSTRUCTIONS=`请先调用 get_report_summary 获取统计汇总，再调用 list_report_items 分页读取事项明细。如果 hasMore 为 true，请继续增加 offset，直至读取完毕。`;

const ACCURACY_RULES=`写作要求：
1. 仅依据 MCP 返回的真实数据撰写，不得补充或推测不存在的事项、结论或成果。
2. 根据 summary.rateMode 准确区分“有效处理率”和“事项办结率”，不得混用统计口径。
3. 综合区间内最新办理结果和 currentStatus 判断已完成与持续推进事项，不能仅依据历史完成时间。
4. 办理说明为空时，只能概括事项名称、类型和状态，不得推测合同修改内容、咨询结论或问题解决结果。
5. 如果发现带“冲突”后缀或其他高度相似的疑似重复事项，统计数字保持 MCP 返回的原值；正文可以合并表述，但必须提示存在待复核数据，不得自行扣减数量。
6. 合并重复或高度相关的工作，避免逐条堆砌；不展示 MCP 未返回的联系人、事项详情或内部备注。
7. 使用正式、清晰、可直接提交的中文表述。周报使用“本周、下周”，月报使用“本月、下月”，其他报告使用“本期、下期”。`;

const BUILT_IN_TEMPLATES:Record<Exclude<ReportTemplateMode,"custom">,string>={
  review:`请使用 In Line MCP，根据真实事项和办理记录生成一份{{报告类型}}。

报告时间范围：{{开始日期}} 至 {{结束日期}}（包含结束日）。

${DATA_INSTRUCTIONS}

请按照以下结构撰写：

一、工作概览
概括本期处理事项总量、完成情况、统计比例口径、主要事项类型和整体工作状态。数据应自然融入文字，避免简单罗列。

二、重点完成事项
归纳本期已经完成并形成明确结果的重点工作。合并重复或高度相关的事项，说明完成内容；办理说明为空时不得推测具体成果。

三、持续推进事项
梳理本期已经处理但尚未结束、仍待材料、待确认或需要继续跟进的工作，说明当前状态和后续关注点。

四、问题与风险
仅根据 MCP 返回的数据总结待办、等待、积压或数据质量问题。数据不足时明确说明，不得虚构原因。

五、下期关注
根据尚未完成事项提出下一阶段建议关注的工作，以“建议关注”或“需继续跟进”表述，不得写成已经确定的工作安排。

${ACCURACY_RULES}`,
  concise:`请使用 In Line MCP，根据真实事项和办理记录生成一份简洁的{{报告类型}}。

报告时间范围：{{开始日期}} 至 {{结束日期}}（包含结束日）。

${DATA_INSTRUCTIONS}

请按照以下结构撰写：

一、本期概览
用一段话说明处理总量、完成情况、准确的统计比例口径和整体进度。

二、重点工作
提炼 3—6 项最有代表性的工作，合并重复或相关事项，突出能够由数据确认的办理结果。

三、后续关注
简要列出仍需推进、等待确认或需要持续关注的事项。全文保持简洁，避免逐条罗列和重复说明。

${ACCURACY_RULES}`,
  byType:`请使用 In Line MCP，根据真实事项和办理记录生成一份按事项类型分类的{{报告类型}}。

报告时间范围：{{开始日期}} 至 {{结束日期}}（包含结束日）。

${DATA_INSTRUCTIONS}

请按照以下结构撰写：

一、总体情况
概括本期处理总量、完成情况、准确的统计比例口径、主要事项类型和整体进度。

二、分类工作情况
按照事项类型归类总结。每个类型说明本期处理数量和完成情况、能够由数据确认的主要办理内容及代表性事项，以及尚未完成或需要继续跟进的工作。

三、后续关注
汇总跨类型的待推进事项、等待事项和可能影响后续工作的风险。同一事项不得重复统计，相似工作应合并表述。

${ACCURACY_RULES}`
};

export function reportTypeForPreset(preset:StatisticsPreset){
  if(preset==="currentWeek")return "本周周报";
  if(preset==="previousWeek")return "上周周报";
  if(preset==="month")return "月报";
  if(preset==="quarter")return "季度工作报告";
  return "阶段工作报告";
}

export function customReportTemplateIsValid(template:string){
  return ["{{报告类型}}","{{开始日期}}","{{结束日期}}"].every(value=>template.includes(value));
}

export function buildReportPrompt({mode,preset,start,end,customTemplate=DEFAULT_CUSTOM_REPORT_TEMPLATE}:{mode:ReportTemplateMode;preset:StatisticsPreset;start:string;end:string;customTemplate?:string}){
  const template=mode==="custom"?customTemplate:BUILT_IN_TEMPLATES[mode];
  return template
    .split("{{报告类型}}").join(reportTypeForPreset(preset))
    .split("{{开始日期}}").join(start)
    .split("{{结束日期}}").join(end);
}
