# Custom Renderer Example

This example demonstrates building a custom JavaScript renderer for rich turn visualization in the CXDB UI.

## What It Is

A React component that displays `com.example.LogEntry` turns with:
- **Level-based styling** (DEBUG/INFO/WARN/ERROR colors)
- **Syntax-highlighted tags** (key-value pairs)
- **Theme support** (light/dark mode)
- **Collapsible raw JSON** view

## Prerequisites

- **Node.js 18+** with npm
- **Running CXDB UI** (for testing)

## Build It

```bash
# Install dependencies
npm install

# Build the renderer
npm run build
```

Output: `dist/renderer.js` (ES module)

## Test Locally

1. **Start a local HTTP server**:
   ```bash
   python3 -m http.server 8001
   ```

2. **Configure the CXDB UI** to load the renderer:

   Edit `frontend/lib/renderers.ts`:
   ```typescript
   export const renderers: RendererConfig[] = [
     {
       type_id: 'com.example.LogEntry',
       renderer_url: 'http://localhost:8001/dist/renderer.js',
       name: 'LogEntry Renderer',
       version: '1.0.0'
     }
   ];
   ```

3. **Run the type-registration example** to create log entries:
   ```bash
   cd ../type-registration
   go run *.go
   ```

4. **View in the UI**:
   ```
   http://localhost:8080/contexts/1
   ```

   The log entries should now render with the custom component.

## Deploy to CDN

### Option 1: AWS S3

```bash
# Upload to S3
aws s3 cp dist/renderer.js s3://your-bucket/renderers/logentry@1.0.0.js --acl public-read

# Update CORS settings
aws s3api put-bucket-cors --bucket your-bucket --cors-configuration file://cors.json
```

**cors.json**:
```json
{
  "CORSRules": [
    {
      "AllowedOrigins": ["*"],
      "AllowedMethods": ["GET"],
      "AllowedHeaders": ["*"]
    }
  ]
}
```

### Option 2: GitHub Pages

```bash
# Copy to docs/ directory
mkdir -p ../../docs/renderers
cp dist/renderer.js ../../docs/renderers/logentry@1.0.0.js

# Commit and push
git add ../../docs/renderers
git commit -m "Add LogEntry renderer"
git push

# Enable GitHub Pages in repository settings (source: docs/)
```

### Option 3: Cloudflare Pages

```bash
# Deploy with Wrangler
npx wrangler pages publish dist
```

## Configure Production

After deploying, update the renderer registry:

**frontend/lib/renderers.ts**:
```typescript
{
  type_id: 'com.example.LogEntry',
  renderer_url: 'https://your-bucket.s3.amazonaws.com/renderers/logentry@1.0.0.js',
  name: 'LogEntry Renderer',
  version: '1.0.0'
}
```

## Renderer API

### Props

```typescript
interface RendererProps {
  // Typed turn data (projected from msgpack via type registry)
  data: {
    timestamp: string;      // ISO-8601 (semantic: unix_ms)
    level: string;          // Enum: DEBUG, INFO, WARN, ERROR
    message: string;
    tags?: Record<string, string>;
  };

  // Turn metadata
  metadata: {
    turn_id: string;
    depth: number;
    declared_type: {
      type_id: string;
      type_version: number;
    };
  };

  // UI theme
  theme: 'light' | 'dark';

  // Error callback (optional)
  onError?: (error: Error) => void;
}
```

### Return Value

Return valid React elements (JSX).

### Example

```jsx
export default function MyRenderer({ data, metadata, theme }) {
  return (
    <div style={{ padding: '1rem' }}>
      <h3>Turn {metadata.turn_id}</h3>
      <p>{data.message}</p>
    </div>
  );
}
```

## Security

### Content Security Policy (CSP)

The CXDB gateway restricts which origins can load renderers.

**Default allowed origins**:
- `https://cdn.strongdm.ai`
- `https://esm.sh`
- `https://cdn.jsdelivr.net`
- `https://unpkg.com`

**Add custom origins**:

Edit `gateway/.env`:
```bash
ALLOWED_RENDERER_ORIGINS=https://cdn.strongdm.ai,https://your-bucket.s3.amazonaws.com
```

Restart the gateway:
```bash
cd ../../gateway
go run ./cmd/server
```

### Renderer Sandboxing

Renderers run in the same browser context as the UI. Follow security best practices:

1. **Validate input**:
   ```jsx
   if (!data || typeof data.message !== 'string') {
     return <div>Invalid data</div>;
   }
   ```

2. **Sanitize HTML**:
   ```jsx
   import DOMPurify from 'https://esm.sh/dompurify@3.0.0';

   const clean = DOMPurify.sanitize(data.html);
   return <div dangerouslySetInnerHTML={{ __html: clean }} />;
   ```

3. **Avoid eval()**:
   - Never use `eval()` or `new Function()`
   - Don't execute code from turn data

4. **Limit network access**:
   - Only load assets from whitelisted CDNs
   - Don't make arbitrary fetch() requests

## Using External Libraries

Import from ESM CDNs:

```jsx
import { LineChart } from 'https://esm.sh/recharts@2.10.0';
import DOMPurify from 'https://esm.sh/dompurify@3.0.0';
```

**Supported CDNs**:
- **esm.sh** (recommended) - transforms npm packages to ESM
- **unpkg.com** - serves npm package files
- **cdn.jsdelivr.net** - fast global CDN
- **skypack.dev** - optimized ESM builds

## Advanced Features

### Lazy Loading

```jsx
import { lazy, Suspense } from 'react';

const HeavyChart = lazy(() => import('https://esm.sh/recharts@2.10.0'));

export default function ChartRenderer({ data }) {
  return (
    <Suspense fallback={<div>Loading chart...</div>}>
      <HeavyChart data={data.points} />
    </Suspense>
  );
}
```

### Memoization

```jsx
import { useMemo } from 'react';

export default function ComputeRenderer({ data }) {
  const processed = useMemo(() => {
    return data.items.map(item => expensiveComputation(item));
  }, [data.items]);

  return <div>{processed.map(renderItem)}</div>;
}
```

### Error Boundaries

```jsx
class ErrorBoundary extends React.Component {
  state = { error: null };

  static getDerivedStateFromError(error) {
    return { error };
  }

  render() {
    if (this.state.error) {
      return <div>Render error: {this.state.error.message}</div>;
    }
    return this.props.children;
  }
}

export default function SafeRenderer({ data }) {
  return (
    <ErrorBoundary>
      <MyActualRenderer data={data} />
    </ErrorBoundary>
  );
}
```

## Troubleshooting

### Renderer Not Loading

**Symptom**: Turn shows "Renderer not found" or default JSON view

**Solutions**:
1. Check that `type_id` matches exactly (case-sensitive)
2. Verify renderer URL is accessible (try in browser)
3. Check browser console for CORS or CSP errors
4. Ensure renderer exports a default function

### CSP Blocks Renderer

**Symptom**: Console error: "Refused to load script from..."

**Solution**: Add renderer origin to `ALLOWED_RENDERER_ORIGINS` in gateway/.env

### Import Errors

**Symptom**: "Failed to resolve module"

**Solutions**:
1. Use full URLs with https:// protocol
2. Use ESM-compatible CDN (esm.sh recommended)
3. Externalize React in vite.config.js

### Renderer Crashes

**Symptom**: "Something went wrong" error in turn card

**Solutions**:
1. Check browser console for error details
2. Add error boundary to renderer
3. Validate input data shape
4. Test with sample data locally

## Development Workflow

1. **Make changes** to `renderer.jsx`
2. **Rebuild**: `npm run build`
3. **Refresh** browser (or use `npm run dev` for watch mode)
4. **Test** with live data from type-registration example

## Next Steps

- **[Type Registration](../type-registration/)**: Create custom types
- **[Renderer Docs](../../docs/renderers.md)**: Complete renderer API reference
- **[Example Renderers](https://github.com/strongdm/cxdb-renderers)**: More examples (Markdown, Charts, Diff)

## Best Practices

1. **Keep renderers small**: Lazy load heavy dependencies
2. **Validate input**: Don't assume data shape
3. **Handle errors gracefully**: Use error boundaries
4. **Test both themes**: Light and dark mode
5. **Optimize performance**: Memoize expensive computations
6. **Version URLs**: Include version in filename (e.g., `@1.0.0.js`)
7. **Document props**: Add JSDoc comments

## License

Copyright 2025 StrongDM Inc
SPDX-License-Identifier: Apache-2.0
