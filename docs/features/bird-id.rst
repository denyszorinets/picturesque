Bird ID
=======

Bird ID suggests the species of a bird in a photo, on your computer, with
the WingCLIP-0.3 model (:doc:`ADR-0018
</architecture/decisions/0018-bird-id-and-non-commercial-models>`).

Setting up
----------

1. In **Settings → AI models**, download **WingCLIP-0.3 bird identifier**
   (79 MB). Its license is CC BY-NC 4.0: non-commercial use only.
2. Optionally, in **Settings → Bird ID**, enter an eBird region code (``US``,
   ``US-WA``, ``US-WA-033``; find yours on ebird.org/explore) and a free
   eBird API key (ebird.org/api/keygen), save, and press **Get the region's
   species**. Suggestions are then limited to the birds eBird lists for the
   region. Without a region, every one of the model's 11,167 species can be
   suggested.

Identifying birds
-----------------

In the full-screen view:

- :kbd:`F` (**Find birds**) finds every bird in the photo with the
  **RT-DETR bird finder** (Apache-2.0, 83 MB, also under **AI models**),
  numbers them on the photo left to right and suggests each one's species.
- :kbd:`D` (**Draw a box**) adds a bird the finder missed: drag a box
  around it.

Click a bird (in the list or its box) to see its five most likely species
with how sure the model is, and click one to add it to the photo. A photo
can carry several species; click a chosen one again, or its ✕, to remove
it. In a stack, **Name all photos of the stack** (on by default) applies
the choice to every frame. With a region set, the closest match outside
the region is mentioned when it differs (a rare visitor, or a wrong guess).

The species show in the details pane and in the **Stacks** pane.

Limits
------

- The identifier always names a bird, even when the box holds none.
- The finder looks at the whole photo at 640 × 640 pixels, so a very small
  bird in a large frame can be missed; draw a box around it.
- Accuracy is measured on North American birds; elsewhere it is less
  certain.
- A low confidence usually means similar species, not a bad box.
