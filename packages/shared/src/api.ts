import { z } from 'zod'

export const planSchema = z.enum(['free', 'pro'])
export type Plan = z.infer<typeof planSchema>

export const captureModeSchema = z.enum(['auto-game', 'display', 'window'])
export type CaptureMode = z.infer<typeof captureModeSchema>

export const cloudStatusSchema = z.enum([
  'local',
  'uploading',
  'live',
  'error',
  'overwritten'
])
export type CloudStatus = z.infer<typeof cloudStatusSchema>

export const clipStatusSchema = z.enum(['reserved', 'live', 'failed'])
export type ClipStatus = z.infer<typeof clipStatusSchema>

export const createClipBodySchema = z.object({
  durationMs: z.number().int().positive(),
  width: z.number().int().positive(),
  height: z.number().int().positive(),
  bytes: z.number().int().nonnegative(),
  mime: z.literal('video/mp4'),
  title: z.string().trim().min(1).max(120).optional()
})
export type CreateClipBody = z.infer<typeof createClipBodySchema>

export const renameClipBodySchema = z.object({
  title: z.string().trim().min(1).max(120)
})
export type RenameClipBody = z.infer<typeof renameClipBodySchema>

export const createClipResponseSchema = z.object({
  id: z.string().length(8),
  publicUrl: z.string().url(),
  videoUploadUrl: z.string().url(),
  thumbUploadUrl: z.string().url()
})
export type CreateClipResponse = z.infer<typeof createClipResponseSchema>

export const completeClipBodySchema = z.object({
  etag: z.string().min(1).optional(),
  replaced: z.boolean().optional()
})
export type CompleteClipBody = z.infer<typeof completeClipBodySchema>

export const meSchema = z.object({
  id: z.string(),
  email: z.string().nullable(),
  plan: planSchema,
  usedBytes: z.number().int().nonnegative(),
  clipCount: z.number().int().nonnegative(),
  freeClipLimit: z.number().int().nonnegative(),
  storageCap: z.number().int().nonnegative()
})
export type Me = z.infer<typeof meSchema>

export const billingIntervalSchema = z.enum(['month', 'year'])
export type BillingInterval = z.infer<typeof billingIntervalSchema>

export const stripeCheckoutBodySchema = z.object({
  interval: billingIntervalSchema
})
export type StripeCheckoutBody = z.infer<typeof stripeCheckoutBodySchema>

export const clipPublicSchema = z.object({
  id: z.string().length(8),
  status: clipStatusSchema,
  revision: z.number().int().positive(),
  durationMs: z.number().int().nullable(),
  width: z.number().int().nullable(),
  height: z.number().int().nullable(),
  title: z.string().nullable(),
  videoUrl: z.string().url(),
  thumbUrl: z.string().url()
})
export type ClipPublic = z.infer<typeof clipPublicSchema>
