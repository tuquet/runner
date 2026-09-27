import fs from 'fs';
import path from 'path';
import { fileURLToPath } from 'url';

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);

const blocksDistPath = path.resolve(__dirname, '../../tuquet-automa/packages/types/dist/blocks/index.js');
const workflowDistPath = path.resolve(__dirname, '../../tuquet-automa/packages/types/dist/workflow.js');
const { createBlockNode } = await import(`file://${blocksDistPath}`);
const { defineWorkflow } = await import(`file://${workflowDistPath}`);

console.log('--- Testing TypeScript Block Schemas & Factory ---');

// 1. Create typed trigger node
const triggerNode = createBlockNode('trigger', {
  type: 'manual',
  parameters: [
    {
      name: 'keyword',
      defaultValue: 'typescript_block_schemas',
      description: 'Search query keyword',
    },
  ],
}, { id: 'node-trigger' });

console.log('✔ Created Trigger Node:', triggerNode.label, triggerNode.id);

// 2. Create typed new-tab node
const newTabNode = createBlockNode('new-tab', {
  url: 'https://example.com/?q={{ variables.keyword }}',
  active: true,
}, { id: 'node-new-tab' });

console.log('✔ Created NewTab Node:', newTabNode.label, newTabNode.data.url);

// 3. Create typed javascript-code node (defaults like preloadScripts: [] and timeout are auto-populated)
const jsCodeNode = createBlockNode('javascript-code', {
  code: 'console.log("TypeScript Block Schema execution success!");',
  context: 'webpage',
  timeout: 5000,
}, { id: 'node-js' });

console.log('✔ Created JS Code Node:', jsCodeNode.label, 'preloadScripts:', jsCodeNode.data.preloadScripts);

// 4. Create typed delay node
const delayNode = createBlockNode('delay', {
  time: 1500,
}, { id: 'node-delay' });

console.log('✔ Created Delay Node:', delayNode.label, delayNode.data.time, 'ms');

// 5. Compose full workflow using defineWorkflow
const workflow = defineWorkflow({
  name: 'E2E Typed Block Schema Workflow',
  description: 'Generated strictly from @automa/types block schemas and factory helpers',
  nodes: [triggerNode, newTabNode, jsCodeNode, delayNode],
  edges: [
    {
      id: 'edge-1',
      source: 'node-trigger',
      target: 'node-new-tab',
      sourceHandle: 'node-trigger-output-1',
      targetHandle: 'node-new-tab-input-1',
    },
    {
      id: 'edge-2',
      source: 'node-new-tab',
      target: 'node-js',
      sourceHandle: 'node-new-tab-output-1',
      targetHandle: 'node-js-input-1',
    },
    {
      id: 'edge-3',
      source: 'node-js',
      target: 'node-delay',
      sourceHandle: 'node-js-output-1',
      targetHandle: 'node-delay-input-1',
    },
  ],
  variables: {
    keyword: 'tuquet_typed_nodes',
  },
});

const outputPath = path.join(__dirname, 'test_typed_generated_workflow.json');
fs.writeFileSync(outputPath, JSON.stringify(workflow, null, 2));
console.log('✔ Workflow successfully written to:', outputPath);
